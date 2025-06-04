#![recursion_limit = "256"]

mod handlers;
mod models;
mod state_store;

use actix_web::{middleware, web, App, HttpServer};
use state_store::create_state_store;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Create shared application state
    let app_state = std::sync::Arc::new(create_state_store().await.unwrap_or_else(|e| {
        eprintln!("Failed to initialize state store: {}", e);
        std::process::exit(1);
    }));

    println!("Starting 3DS Mock Server on http://localhost:8080");
    println!("Available endpoints:");
    println!("  POST /3ds/version");
    println!("  POST /3ds/authenticate");
    println!("  POST /3ds/results");
    println!("  POST /3ds/final");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .route("/3ds/version", web::post().to(handlers::version_handler))
            .route("/3ds/authenticate", web::post().to(handlers::authenticate_handler))
            .route("/3ds/results", web::post().to(handlers::results_handler))
            .route("/3ds/final", web::post().to(handlers::final_handler))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
