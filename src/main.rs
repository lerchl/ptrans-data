use axum::{
    BoxError, Json, Router,
    error_handling::HandleErrorLayer,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool};
use std::{env, time::Duration};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Debug, Serialize, Clone, FromRow)]
struct Lio {
    id: String,
    provider: String,
    station: String,
    line: String,
    direction: String,
}

#[derive(Debug, Deserialize)]
struct LioCreateDto {
    provider: String,
    station: String,
    line: String,
    direction: String,
}

async fn get_lio(State(pool): State<MySqlPool>) -> Result<Json<Vec<Lio>>, StatusCode> {
    let lios = sqlx::query_as::<_, Lio>("SELECT id, provider, station, line, direction FROM lios")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(lios))
}
async fn create_lio(
    State(pool): State<MySqlPool>,
    Json(input): Json<LioCreateDto>,
) -> Result<(StatusCode, Json<Lio>), StatusCode> {
    let id = Uuid::new_v4().to_string();

    sqlx::query!(
        r#"
        INSERT INTO lios (id, provider, station, line, direction)
        VALUES (?, ?, ?, ?, ?)
        "#,
        id,
        input.provider,
        input.station,
        input.line,
        input.direction
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let lio = Lio {
        id,
        provider: input.provider,
        station: input.station,
        line: input.line,
        direction: input.direction,
    };

    Ok((StatusCode::CREATED, Json(lio)))
}

async fn delete_lio(
    State(pool): State<MySqlPool>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let res = sqlx::query!("DELETE FROM lios WHERE id = ?", id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}
#[tokio::main]
async fn main() {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to MariaDB");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Compose the routes
    let app = Router::new()
        .route("/lio", get(get_lio).post(create_lio))
        .route("/lio/{id}", delete(delete_lio))
        // Add middleware to all routes
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
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}
