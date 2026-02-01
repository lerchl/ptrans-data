use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize)]
pub struct ErrorDto {
    pub message: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct LioViewDto {
    pub id: String,
    pub provider: String,
    pub station: String,
    pub line: String,
    pub direction: String,
}

#[derive(Debug, Deserialize)]
pub struct LioCreateDto {
    pub provider: String,
    pub station: String,
    pub line: String,
    pub direction: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimetableDto {
    pub trips: Vec<TripDto>,
    pub message: Option<String>
}

#[derive(Clone, Debug, Serialize)]
pub struct TripDto {
    pub line: String,
    pub direction: String,
    pub foot_minutes_to_station: i32,
    pub departures: Vec<DepartureDto>
}

#[derive(Clone, Debug, Serialize)]
pub struct DepartureDto {
    pub direction: String,
    pub when: String,
    pub when_actually: Option<String>,
    pub traffic_jam: bool
}
