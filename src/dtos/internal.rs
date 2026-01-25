use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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
