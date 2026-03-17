mod dtos;
mod models;
mod services;

use axum::{
    BoxError, Router,
    error_handling::HandleErrorLayer,
    http::StatusCode,
    routing::{delete, get},
};
use dotenvy::dotenv;
use sqlx::MySqlPool;
use std::{env, time::Duration};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    models::internal::Station,
    services::{
        internal::{create_lio, delete_lio, get_lio, get_timetable},
        wl,
    },
};

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
    stations: Vec<Station>,
}

#[tokio::main]
async fn main() {
    println!("before dotenv");
    dotenv().ok();

    println!("before registering trace");
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("before getting wl stations");
    let stations = match wl::get_stations().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to get stations: {e}");
            Vec::new()
        }
    };

    println!("before getting database url env");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    println!("before connecting to maria db");
    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to MariaDB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to apply database migrations: {}", e);
            panic!("Migration failed, shutting down");
        });

    let state = AppState {
        pool: pool.clone(),
        stations,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/timetable", get(get_timetable))
        .route("/lio", get(get_lio).post(create_lio))
        .route("/lio/{id}", delete(delete_lio))
        .layer(cors)
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {error}"),
                        ))
                    }
                }))
                .timeout(Duration::from_secs(60))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}
