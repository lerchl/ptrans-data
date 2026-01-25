mod dtos;
mod models;

use axum::{
    BoxError, Json, Router,
    error_handling::HandleErrorLayer,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
};
use csv::ReaderBuilder;
use dotenvy::dotenv;
use reqwest::Client;
use serde::Serialize;
use sqlx::MySqlPool;
use std::{env, time::Duration};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::{
    dtos::internal::{LioCreateDto, LioViewDto},
    models::{
        internal::{Lio, Station},
        wl::{ApiResponse, Monitor, StationCsvRow},
    },
};

async fn fetch_and_parse_csv(url: &str) -> Result<Vec<Station>, Box<dyn std::error::Error>> {
    // Fetch CSV as string
    let resp = Client::new().get(url).send().await?.text().await?;

    // Create CSV reader
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .from_reader(resp.as_bytes());

    // Deserialize into Vec<LioCsvRow>
    let mut rows = Vec::new();
    for result in rdr.deserialize() {
        let row: StationCsvRow = result?;
        rows.push(Station {
            id: row.diva,
            name: row.platform_text,
            provider: "Wiener Linien".to_string(),
        });
    }

    Ok(rows)
}

#[derive(Clone, Debug, Serialize)]
struct ErrorBody {
    message: String,
}

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
    stations: Vec<Station>,
}

async fn get_lio(State(app_state): State<AppState>) -> Result<Json<Vec<LioViewDto>>, StatusCode> {
    let lios = sqlx::query_as::<_, LioViewDto>("SELECT id, provider, station, line, direction FROM lios")
        .fetch_all(&app_state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(lios))
}
async fn create_lio(
    State(app_state): State<AppState>,
    Json(input): Json<LioCreateDto>,
) -> Result<(StatusCode, Json<LioViewDto>), (StatusCode, Json<ErrorBody>)> {
    let found_stations = app_state
        .stations
        .iter()
        .filter(|s| {
            s.name
                .to_lowercase()
                .contains(&input.station.to_lowercase())
                && s.provider == input.provider
        })
        .collect::<Vec<&Station>>();

    if found_stations.len() == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                message: format!(
                    "Station '{}' with provider '{}' not found.",
                    input.station, input.provider
                ),
            }),
        ));
    } else if found_stations.len() > 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                message: format!(
                    "Multiple stations found matching '{}' with provider '{}'. Please be more specific.",
                    input.station, input.provider
                ),
            }),
        ));
    }

    let station = found_stations[0];

    let resp = Client::new()
        .get(format!(
            "https://www.wienerlinien.at/ogd_realtime/monitor?diva={}",
            station.id
        ))
        .send()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    message: "Failed to fetch data from Wiener Linien API.".to_string(),
                }),
            )
        })?
        .json::<ApiResponse>()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    message: "Failed to parse response from Wiener Linien API.".to_string(),
                }),
            )
        })?;

    let lines = resp
        .data
        .monitors
        .iter()
        .filter(|monitor| {
            monitor.lines.iter().any(|line| {
                line.name.to_lowercase() == input.line.to_lowercase()
                    && line
                        .towards
                        .to_lowercase()
                        .contains(&input.direction.to_lowercase())
            })
        })
        .collect::<Vec<&Monitor>>();

    if lines.len() == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                message: format!(
                    "Line '{}' with direction '{}' not found at station '{}'.",
                    input.line, input.direction, station.name
                ),
            }),
        ));
    }

    println!("{}, {}", lines[0].lines[0].name, lines[0].lines[0].towards);

    let id = Uuid::new_v4().to_string();

    sqlx::query!(
        r#"
        INSERT INTO lios (id, provider, provider_id, station, line, direction)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
        id,
        input.provider,
        station.id,
        input.station,
        input.line,
        input.direction
    )
    .execute(&app_state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);

    Ok((
        StatusCode::CREATED,
        Json(LioViewDto {
            id,
            provider: input.provider,
            station: input.station,
            line: input.line,
            direction: input.direction,
        }),
    ))
}

async fn delete_lio(
    State(app_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let res = sqlx::query!("DELETE FROM lios WHERE id = ?", id)
        .execute(&app_state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}
#[tokio::main]
async fn main() {
    let stations = fetch_and_parse_csv(
        "https://www.wienerlinien.at/ogd_realtime/doku/ogd/wienerlinien-ogd-haltestellen.csv",
    )
    .await
    .expect("Failed to fetch or parse CSV");

    stations.iter().for_each(|station| {
        println!(
            "Station ID: {}, Name: {}, Provider: {}",
            station.id, station.name, station.provider
        );
    });

    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to MariaDB");

    let state = AppState {
        pool: pool.clone(),
        stations,
    };

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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    let _ = axum::serve(listener, app).await;
}
