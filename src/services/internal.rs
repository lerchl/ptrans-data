use axum::{
    Json,
    extract::{Path, State},
};
use reqwest::{Client, StatusCode};
use uuid::Uuid;

use crate::{
    AppState,
    dtos::internal::{ErrorDto, LioCreateDto, LioViewDto, TimetableDto},
    models::{
        internal::{IntervalLio, Station},
        oebb::Departure,
        wl::{Monitor, MonitorResponse},
    },
    services::{oebb, wl},
};

pub async fn get_lio(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<LioViewDto>>, StatusCode> {
    let lios =
        sqlx::query_as::<_, LioViewDto>("SELECT id, provider, station, line, direction FROM lios")
            .fetch_all(&app_state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(lios))
}

pub async fn create_lio(
    State(app_state): State<AppState>,
    Json(input): Json<LioCreateDto>,
) -> Result<(StatusCode, Json<LioViewDto>), (StatusCode, Json<ErrorDto>)> {
    if input.provider.as_str() == "Wiener Linien" {
        let mut found_stations = app_state
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
                Json(ErrorDto {
                    message: format!(
                        "Station '{}' with provider '{}' not found.",
                        input.station, input.provider
                    ),
                }),
            ));
        } else if found_stations.len() > 1 {
            let exact_maches = found_stations
                .iter()
                .filter(|s| s.name.to_lowercase() == input.station.to_lowercase())
                .collect::<Vec<&&Station>>();

            if exact_maches.len() == 1 {
                found_stations = vec![exact_maches[0]];
            } else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorDto {
                        message: format!(
                            "Multiple stations found matching '{}' with provider '{}'. Please be more specific. Found stations: {:?}",
                            input.station,
                            input.provider,
                            found_stations
                                .iter()
                                .map(|s| &s.name)
                                .collect::<Vec<&String>>()
                        ),
                    }),
                ));
            }
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
                    Json(ErrorDto {
                        message: "Failed to fetch data from Wiener Linien API.".to_string(),
                    }),
                )
            })?
            .json::<MonitorResponse>()
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorDto {
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
                Json(ErrorDto {
                    message: format!(
                        "Line '{}' with direction '{}' not found at station '{}'.",
                        input.line, input.direction, station.name
                    ),
                }),
            ));
        }

        let id = Uuid::new_v4().to_string();

        let create_result = sqlx::query!(
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
        .await;

        return match create_result {
            Ok(_) => Ok((
                StatusCode::CREATED,
                Json(LioViewDto {
                    id,
                    provider: input.provider,
                    station: input.station,
                    line: input.line,
                    direction: input.direction,
                }),
            )),
            Err(_e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorDto {
                    message: "Failed to create LIO.".to_string(),
                }),
            )),
        };
    } else if input.provider.as_str() == "OEBB" {
        let stations = oebb::fetch_stations(input.station.clone()).await.unwrap();

        if stations.len() > 1 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorDto {
                    message: format!(
                        "Multiple stations found matching '{}' with provider '{}'. Please be more specific. Found stations: {:?}",
                        input.station,
                        input.provider,
                        stations.iter().map(|s| &s.name).collect::<Vec<&String>>()
                    ),
                }),
            ));
        }

        let departures = oebb::fetch_depatures_for_stations(vec![stations[0].id.clone()])
            .await
            .map_err(|e| {
                println!("Error fetching departures: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorDto {
                        message: format!(
                            "Failed to fetch departures from OEBB API: {}",
                            e.to_string()
                        ),
                    }),
                )
            })?;

        let desired_departures = departures
            .iter()
            .filter(|d| {
                d.direction
                    .to_lowercase()
                    .contains(&input.direction.to_lowercase())
                    && d.line
                        .name
                        .replace(" ", "")
                        .to_lowercase()
                        .contains(&input.line.to_lowercase())
            })
            .collect::<Vec<&Departure>>();

        if desired_departures.len() == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorDto {
                    message: format!(
                        "Line '{}' with direction '{}' not found at station '{}'.",
                        input.line, input.direction, stations[0].name
                    ),
                }),
            ));
        }

        let id = Uuid::new_v4().to_string();

        let create_result = sqlx::query!(
            r#"
        INSERT INTO lios (id, provider, provider_id, station, line, direction)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
            id,
            input.provider,
            stations[0].id,
            input.station,
            input.line,
            input.direction
        )
        .execute(&app_state.pool)
        .await;

        return match create_result {
            Ok(_) => Ok((
                StatusCode::CREATED,
                Json(LioViewDto {
                    id,
                    provider: input.provider,
                    station: input.station,
                    line: input.line,
                    direction: input.direction,
                }),
            )),
            Err(_e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorDto {
                    message: "Failed to create LIO.".to_string(),
                }),
            )),
        };
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorDto {
                message: format!("Provider '{}' not supported", input.provider),
            }),
        ));
    }
}

pub async fn delete_lio(
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

pub async fn get_timetable(
    State(app_state): State<AppState>,
) -> Result<(StatusCode, Json<TimetableDto>), StatusCode> {
    let lios =
        sqlx::query_as::<_, IntervalLio>("SELECT provider, provider_id, line, direction FROM lios")
            .fetch_all(&app_state.pool)
            .await
            .map_err(|e| {
                eprintln!("{:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    let wl_lios = lios
        .iter()
        .filter(|lio| lio.provider.as_str() == "Wiener Linien")
        .collect::<Vec<&IntervalLio>>();

    // let oebb_lios = lios
    //     .iter()
    //     .filter(|lio| lio.provider.as_str() == "OEBB")
    //     .collect::<Vec<&IntervalLio>>();

    let wl_trips = wl::fetch_trips_for_lios(&wl_lios).await.map_err(|e| {
        eprintln!("{:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::OK,
        Json(TimetableDto {
            trips: wl_trips,
            message: None,
        }),
    ))
}
