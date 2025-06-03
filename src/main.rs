#![recursion_limit = "256"]

mod handlers;
mod models;
mod state;

use actix_web::{middleware, web, App, HttpServer};
use state::create_app_state;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Create shared application state
    let app_state = create_app_state();

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
