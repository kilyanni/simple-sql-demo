mod db;
mod error;
mod routes;
mod views;

use axum::{Router, routing::get};
use sqlx::{PgPool, postgres::PgConnectOptions};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Local peer authentication by default; DATABASE_URL can select a remote server.
    let options = match std::env::var("DATABASE_URL") {
        Ok(url) => url.parse::<PgConnectOptions>()?,
        Err(_) => PgConnectOptions::new()
            .socket("/run/postgresql")
            .username(&std::env::var("USER").unwrap_or_else(|_| "postgres".into()))
            .database("moodle"),
    };
    let pool = PgPool::connect_with(options).await?;

    let app = Router::new()
        .route("/", get(routes::overview))
        .route("/assignments", get(routes::assignments))
        .route("/quizzes", get(routes::quizzes))
        .route("/forums", get(routes::forums))
        .route("/student", get(routes::student))
        .route("/course", get(routes::course))
        .route("/notifications", get(routes::notifications))
        .route("/static/pico.min.css", get(routes::pico_css))
        .route("/static/app.css", get(routes::app_css))
        .fallback(routes::not_found)
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}
