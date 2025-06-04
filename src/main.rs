#![recursion_limit = "256"]

mod config;
mod handlers;
mod models;
mod state_store;

use actix_web::{middleware, web, App, HttpServer, HttpResponse, Result};
use actix_web_prom::PrometheusMetricsBuilder;
use actix_governor::{Governor, GovernorConfigBuilder};
use state_store::{create_redis_store, StateStore};
use std::sync::Arc;
use std::time::Duration;

// Health check endpoint
async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "3ds-mock-server"
    })))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load configuration
    let settings = config::Settings::new().unwrap_or_else(|e| {
        eprintln!("❌ Failed to load configuration: {}", e);
        eprintln!("Make sure config/development.toml or config/production.toml exists and is valid.");
        std::process::exit(1);
    });

    // Validate configuration
    if let Err(e) = settings.validate() {
        eprintln!("❌ Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    // Initialize logger with configured level
    env_logger::init_from_env(
        env_logger::Env::new().default_filter_or(&settings.server.log_level)
    );

    let run_mode = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
    println!("🚀 Starting 3DS Mock Server (Production Optimized)");
    println!("📁 Configuration mode: {}", run_mode);
    println!("⚡ Performance features:");
    println!("   🗜️  Compression: {}", if settings.performance.enable_compression { "enabled" } else { "disabled" });
    println!("   📊 Metrics: {}", if settings.performance.enable_metrics { "enabled" } else { "disabled" });
    println!("   🚦 Rate limiting: {} req/s", settings.performance.rate_limit_per_second);

    // Create Redis store (Redis-only, no fallback)
    let redis_store = create_redis_store(&settings).await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to initialize Redis store: {}", e);
        eprintln!("🔧 Redis is required for this application to run.");
        eprintln!("   Please ensure Redis is running at: {}", settings.redis.url);
        std::process::exit(1);
    });

    let app_state: Arc<Box<dyn StateStore>> = Arc::new(Box::new(redis_store));
    let server_addr = settings.server_address();

    // Setup Prometheus metrics if enabled
    let prometheus = if settings.performance.enable_metrics {
        Some(
            PrometheusMetricsBuilder::new("api")
                .endpoint(&settings.monitoring.metrics_endpoint)
                .build()
                .unwrap()
        )
    } else {
        None
    };

    // Setup rate limiting
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(settings.performance.rate_limit_per_second as u64)
        .burst_size(settings.performance.rate_limit_per_second * 2) // Allow bursts up to 2x the rate
        .finish()
        .unwrap();

    println!("🌐 Server starting on: http://{}", server_addr);
    println!("📋 Available endpoints:");
    println!("   POST /3ds/version");
    println!("   POST /3ds/authenticate");
    println!("   POST /3ds/results");
    println!("   POST /3ds/final");
    if settings.performance.enable_metrics {
        println!("   GET  {} (metrics)", settings.monitoring.metrics_endpoint);
    }
    println!("   GET  {} (health)", settings.monitoring.health_endpoint);

    let settings_clone = settings.clone();
    
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(settings_clone.clone()))
            .wrap(middleware::Logger::default())
            .wrap(Governor::new(&governor_conf))
            .wrap(middleware::Compress::default())
            .route(&settings_clone.monitoring.health_endpoint, web::get().to(health_check))
            .route("/3ds/version", web::post().to(handlers::version_handler))
            .route("/3ds/authenticate", web::post().to(handlers::authenticate_handler))
            .route("/3ds/results", web::post().to(handlers::results_handler))
            .route("/3ds/final", web::post().to(handlers::final_handler))
    })
    .workers(settings.server.workers.unwrap_or(0)) // 0 = use all CPU cores
    .client_request_timeout(Duration::from_millis(settings.performance.client_timeout_ms))
    .keep_alive(Duration::from_secs(settings.performance.keep_alive_seconds))
    .bind(&server_addr)?
    .run()
    .await
}
