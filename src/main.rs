#![recursion_limit = "256"]

mod config;
mod handlers;
mod models;
mod state_store;

use actix_web::{middleware, web, App, HttpServer};
use state_store::{create_redis_store, StateStore};
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load configuration
    let settings = config::Settings::new().unwrap_or_else(|e| {
        eprintln!("❌ Failed to load configuration: {}", e);
        eprintln!("Make sure config/default.toml exists and is valid.");
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
    println!("🚀 Starting 3DS Mock Server");
    println!("📁 Configuration mode: {}", run_mode);

    // Create Redis store (Redis-only, no fallback)
    let redis_store = create_redis_store(&settings).await.unwrap_or_else(|e| {
        eprintln!("❌ Failed to initialize Redis store: {}", e);
        eprintln!("🔧 Redis is required for this application to run.");
        eprintln!("   Please ensure Redis is running at: {}", settings.redis.url);
        std::process::exit(1);
    });

    let app_state: Arc<Box<dyn StateStore>> = Arc::new(Box::new(redis_store));
    let server_addr = settings.server_address();

    println!("🌐 Server starting on: http://{}", server_addr);
    println!("📋 Available endpoints:");
    println!("   POST /3ds/version");
    println!("   POST /3ds/authenticate");
    println!("   POST /3ds/results");
    println!("   POST /3ds/final");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .route("/3ds/version", web::post().to(handlers::version_handler))
            .route("/3ds/authenticate", web::post().to(handlers::authenticate_handler))
            .route("/3ds/results", web::post().to(handlers::results_handler))
            .route("/3ds/final", web::post().to(handlers::final_handler))
    })
    .bind(&server_addr)?
    .run()
    .await
}
