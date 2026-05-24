mod error;
mod models;
mod routes;
mod services;
mod state;

use anyhow::Context;
use axum::http::{HeaderValue, Method};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn,tower_http=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL env var required")?;
    let bind_addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()
        .context("invalid BIND_ADDR")?;
    let cors_origin = std::env::var("CORS_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".into());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database_url)
        .await
        .context("connecting to postgres")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running migrations")?;

    let _scheduler = services::jobs::start_scheduler(pool.clone())
        .await
        .context("starting alerts scheduler")?;

    let whatsapp = services::whatsapp::WhatsappService::from_env(pool.clone());
    let state = AppState { db: pool, whatsapp };

    let cors = CorsLayer::new()
        .allow_origin(cors_origin.parse::<HeaderValue>()?)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any);

    let app = routes::router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    tracing::info!("listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}
