use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Location {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String
}

#[derive(Debug, Deserialize)]
pub struct Departures {
    pub departures: Vec<Departure>
}

#[derive(Debug, Deserialize, Clone)]
pub struct Departure {
    pub when: String,
    // #[serde(rename = "plannedWhen")]
    // pub planned_when: String,
    /// delay in seconds
    pub delay: Option<i32>,
    pub direction: String,
    pub line: Line
}

#[derive(Debug, Deserialize, Clone)]
pub struct Line {
    pub name: String
}
