mod dtos;
mod models;
mod services;

use axum::{
    BoxError, Json, Router,
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
    dtos::internal::VersionDto,
    models::internal::Station,
    services::{
        internal::{create_lio, delete_lio, get_lio, get_timetable},
        spotify, wl,
    },
};

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
    stations: Vec<Station>,
    spotify_credentials: rspotify::Credentials,
    spotify_oauth_template: rspotify::OAuth,
}

async fn version() -> Json<VersionDto> {
    Json(VersionDto {
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let spotify_client_id = env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID must be set");
    let spotify_client_secret =
        env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET must be set");

    let creds = rspotify::Credentials::new(&spotify_client_id, &spotify_client_secret);
    let oauth = rspotify::OAuth {
        redirect_uri: "http://192.168.10.24:3000/spotify/callback".to_string(),
        scopes: rspotify::scopes!("user-read-playback-state"),
        state: "12345678".to_string(),
        ..Default::default()
    };

    let port = env::var("PORT").expect("PORT must be set");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let stations = match wl::get_stations().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to get stations: {e}");
            Vec::new()
        }
    };

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
        spotify_credentials: creds,
        spotify_oauth_template: oauth,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/version", get(version))
        .route("/timetable", get(get_timetable))
        .route("/lio", get(get_lio).post(create_lio))
        .route("/lio/{id}", delete(delete_lio))
        .route("/spotify/authorize", get(spotify::get_authorize_redirect))
        .route("/spotify/callback", get(spotify::callback))
        .route("/spotify/users", get(spotify::get_users))
        .route(
            "/spotify/currentlyPlaying",
            get(spotify::get_currently_playing),
        )
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

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}
