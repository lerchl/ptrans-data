use chrono::NaiveDateTime;
use sqlx::FromRow;

#[derive(Clone, Debug)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub provider: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct IntervalLio {
    pub provider: String,
    pub provider_id: String,
    pub line: String,
    pub direction: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SpotifyUser {
    pub user_id: String,
    pub display_name: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: NaiveDateTime,
    pub scopes: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SpotifyUserView {
    pub display_name: String,
    pub expires_at: NaiveDateTime,
    pub scopes: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SpotifyTokenView {
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: NaiveDateTime,
    pub scopes: String,
}
