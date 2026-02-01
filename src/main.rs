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
use sqlx::{MySql, MySqlPool, Pool};
use std::{env, time::Duration};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    models::internal::{IntervalLio, Station},
    services::{
        internal::{create_lio, delete_lio, get_lio, get_timetable},
        oebb, wl,
    },
};

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
    stations: Vec<Station>,
}

/// Fetches the timetable for all LIOs in the database and prints them to the console.
async fn get_and_print_timetable(pool: &Pool<MySql>) -> Result<(), Box<dyn std::error::Error>> {
    let lios =
        sqlx::query_as::<_, IntervalLio>("SELECT provider, provider_id, line, direction FROM lios")
            .fetch_all(pool)
            .await?;

    let wl_lios = lios
        .iter()
        .filter(|lio| lio.provider.as_str() == "Wiener Linien")
        .collect::<Vec<&IntervalLio>>();
    let divas = wl_lios
        .iter()
        .map(|lio| lio.provider_id.clone())
        .collect::<Vec<String>>();

    let oebb_lios = lios
        .iter()
        .filter(|lio| lio.provider.as_str() == "OEBB")
        .collect::<Vec<&IntervalLio>>();
    let oebb_ids = oebb_lios
        .iter()
        .map(|lio| lio.provider_id.clone())
        .collect::<Vec<String>>();

    let wl_result = wl::fetch_monitors(divas).await?;
    let oebb_result = oebb::fetch_depatures_for_stations(oebb_ids).await?;

    println!("--- Timetable Update ---");
    for line in [
        wl::format_monitors_plain(&wl::filter_monitors_for_lios(
            &wl_result.data.monitors,
            &wl_lios,
        )),
        oebb::format_departures_plain(&oebb::filter_departures_for_lios(&oebb_result, &oebb_lios)),
    ]
    .concat()
    {
        println!("{line}");
    }
    println!("------------------------\n");

    Ok(())
}

#[tokio::main]
async fn main() {
    let stations = wl::get_stations().await.unwrap();

    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = MySqlPool::connect(&database_url)
        .await
        .expect("Failed to connect to MariaDB");

    // let interval_pool = pool.clone();
    // tokio::spawn(async move {
    //     loop {
    //         if let Err(e) = get_and_print_timetable(&interval_pool).await {
    //             eprintln!("Error fetching timetable: {}", e);
    //         }
    //         tokio::time::sleep(Duration::from_secs(30)).await;
    //     }
    // });

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

    let app = Router::new()
        .route("/timetable", get(get_timetable))
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
                .timeout(Duration::from_secs(60))
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
