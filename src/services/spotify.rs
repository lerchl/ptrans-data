use futures::future::join_all;
use std::collections::HashSet;

use axum::{
    Json,
    extract::{Query, State},
    response::Redirect,
};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use rspotify::{
    AuthCodeSpotify, Token,
    prelude::{BaseClient, OAuthClient},
};
use serde::Deserialize;

use crate::{
    AppState,
    dtos::internal::{CurrentlyPlayingDto, ErrorDto, SpotifyUserViewDto},
    models::{
        internal::{SpotifyTokenView, SpotifyUserView},
        spotify::PlayerResponse,
    },
};

pub async fn get_users(
    State(app_state): State<AppState>,
) -> Result<(StatusCode, Json<Vec<SpotifyUserViewDto>>), StatusCode> {
    let user_views = sqlx::query_as::<_, SpotifyUserView>(
        "SELECT display_name, expires_at, scopes FROM spotify_users",
    )
    .fetch_all(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Could not fetch spotify users to view: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::OK,
        Json(
            user_views
                .iter()
                .map(|u| SpotifyUserViewDto {
                    display_name: u.display_name.clone(),
                    auth_expires_at: u.expires_at,
                    fully_scoped: true,
                })
                .collect::<Vec<SpotifyUserViewDto>>(),
        ),
    ))
}

pub async fn get_authorize_redirect(
    State(app_state): State<AppState>,
) -> Result<Redirect, (StatusCode, Json<ErrorDto>)> {
    let spotify = AuthCodeSpotify::new(
        app_state.spotify_credentials.clone(),
        app_state.spotify_oauth_template.clone(),
    );

    match spotify.get_authorize_url(false) {
        Err(client_error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                message: client_error.to_string(),
            }),
        )),
        Ok(url) => Ok(Redirect::to(&url)),
    }
}

#[derive(Deserialize, Debug)]
pub struct CallbackParams {
    code: Option<String>,
    error: Option<String>,
    state: Option<String>,
}

pub async fn callback(
    Query(params): Query<CallbackParams>,
    State(app_state): State<AppState>,
) -> Result<StatusCode, (StatusCode, Json<ErrorDto>)> {
    let state = params.state.ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorDto {
                message: "No state present!".to_string(),
            }),
        )
    })?;

    if state != "12345678" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorDto {
                message: "States do not match!".to_string(),
            }),
        ));
    }

    if let Some(error) = params.error {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorDto { message: error })));
    }

    let code = params.code.ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorDto {
                message: "No error or code present!".to_string(),
            }),
        )
    })?;

    let spotify = AuthCodeSpotify::new(
        app_state.spotify_credentials.clone(),
        app_state.spotify_oauth_template.clone(),
    );

    let request_token_result = spotify.request_token(&code).await;

    if let Err(error) = request_token_result {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                message: error.to_string(),
            }),
        ));
    }

    let token_mutex = spotify.get_token();
    let token_guard = token_mutex.lock().await.map_err(|_e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                message: "Could not lock on token.".to_string(),
            }),
        )
    })?;

    let token = token_guard.clone().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                message: "Token not set.".to_string(),
            }),
        )
    })?;

    tracing::debug!("Got token");

    let user_spotify = AuthCodeSpotify::from_token(token.clone());

    tracing::debug!("Getting profile...");
    let profile = user_spotify.current_user().await;
    tracing::debug!("Got profile");

    match profile {
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                message: error.to_string(),
            }),
        )),
        Ok(user) => {
            let result = sqlx::query!(
                r#"
                INSERT INTO spotify_users (
                    user_id, display_name, access_token, refresh_token, expires_at, scopes
                ) VALUES (?, ?, ?, ?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    display_name = VALUES(display_name),
                    access_token = VALUES(access_token),
                    refresh_token = VALUES(refresh_token),
                    expires_at = VALUES(expires_at),
                    scopes = VALUES(scopes)"#,
                user.id.to_string(),
                user.display_name,
                token.access_token,
                token.refresh_token,
                token.expires_at.map(|dt| dt.naive_utc()),
                token.scopes.iter().cloned().collect::<Vec<_>>().join(" ")
            )
            .execute(&app_state.pool)
            .await;

            match result {
                Ok(_) => Ok(StatusCode::OK),
                Err(e) => {
                    tracing::error!("Could not add to spotify_users: {:?}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorDto {
                            message: "Failed to add spotify user.".to_string(),
                        }),
                    ))
                }
            }
        }
    }
}

async fn get_player_state(access_token: &str) -> Result<Option<PlayerResponse>, reqwest::Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.spotify.com/v1/me/player")
        .bearer_auth(access_token)
        .send()
        .await?;

    // Spotify returns 204 No Content if nothing is playing at all
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }

    let player = response.json::<PlayerResponse>().await?;
    Ok(Some(player))
}

pub async fn get_currently_playing(
    State(app_state): State<AppState>,
) -> Result<(StatusCode, Json<CurrentlyPlayingDto>), (StatusCode, Json<ErrorDto>)> {
    let rows = sqlx::query_as::<_, SpotifyTokenView>(
        "SELECT user_id, access_token, refresh_token, expires_at, scopes FROM spotify_users",
    )
    .fetch_all(&app_state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Could not fetch spotify user tokens: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorDto {
                message: "Undisclosed".to_string(),
            }),
        )
    })?;

    let futures = rows.into_iter().map(|row| {
        let app_state = app_state.clone();
        async move {
            let scopes: HashSet<String> = row.scopes.split_whitespace().map(String::from).collect();
            let expires_at: DateTime<Utc> = DateTime::from_naive_utc_and_offset(row.expires_at, Utc);

            let mut token = Token {
                access_token: row.access_token.clone(),
                refresh_token: Some(row.refresh_token.clone()),
                expires_at: Some(expires_at),
                expires_in: expires_at.signed_duration_since(Utc::now()),
                scopes,
            };

            let spotify = AuthCodeSpotify::new(
                app_state.spotify_credentials.clone(),
                app_state.spotify_oauth_template.clone(),
            );

            {
                let token_mutex = spotify.get_token();
                let mut guard = token_mutex.lock().await.unwrap();
                *guard = Some(token.clone());
            }

            let is_expired = {
                let token_mutex = spotify.get_token();
                let guard = token_mutex.lock().await.ok()?;
                guard.as_ref().map(|t| t.is_expired()).unwrap_or(true)
            };

            if is_expired {
                if let Err(e) = spotify.refresh_token().await {
                    tracing::error!("Failed to refresh token for user {}: {:?}", row.user_id, e);
                    return None;
                }

                token = {
                    let token_mutex = spotify.get_token();
                    let guard = token_mutex.lock().await.ok()?;
                    guard.clone()?
                };

                if let Err(e) = sqlx::query!(
                    "UPDATE spotify_users SET access_token = ?, refresh_token = ?, expires_at = ? WHERE user_id = ?",
                    token.access_token,
                    token.refresh_token,
                    token.expires_at.map(|dt| dt.naive_utc()),
                    row.user_id,
                )
                .execute(&app_state.pool)
                .await
                {
                    tracing::error!("Failed to persist refreshed token for user {}: {:?}", row.user_id, e);
                }
            }

            match get_player_state(token.access_token.as_str()).await {
                Ok(k) => k,
                Err(e) => {
                    tracing::error!("Error getting current player state for user {}: {:?}", row.user_id, e);
                    None
                }
            }
        }
    });

    let first = join_all(futures)
        .await
        .into_iter()
        .flatten()
        .find(|res| res.device.name == "Living Room");

    match first {
        Some(player_response) => Ok((
            StatusCode::OK,
            Json(CurrentlyPlayingDto {
                is_paused: !player_response.is_playing,
                album_cover_url: player_response.item.and_then(|i| {
                    i.album
                        .images
                        .iter()
                        .filter(|i| i.width.map(|w| w >= 64).unwrap_or(true))
                        .last()
                        .map(|i| i.url.clone())
                }),
            }),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorDto {
                message: "Nothing currently playing on 'Living Room'".to_string(),
            }),
        )),
    }
}
