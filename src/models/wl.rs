use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StationCsvRow {
    #[serde(rename = "DIVA")]
    pub diva: String,
    #[serde(rename = "PlatformText")]
    pub platform_text: String,
}

#[derive(Debug, Deserialize)]
pub struct MonitorResponse {
    pub data: Data,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub monitors: Vec<Monitor>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Monitor {
    pub lines: Vec<Line>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Line {
    pub name: String,
    pub towards: String,
    pub departures: Departures,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Departures {
    pub departure: Vec<Departure>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Departure {
    #[serde(rename = "departureTime")]
    pub departure_time: DepartureTime,
    pub vehicle: Option<Vehicle>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DepartureTime {
    #[serde(rename = "timePlanned")]
    pub time_planned: String,
    #[serde(rename = "timeReal")]
    pub time_real: Option<String>,
    pub countdown: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Vehicle {
    pub towards: String,
    #[serde(rename = "realtimeSupported")]
    pub realtime_supported: bool,
    #[serde(rename = "trafficjam")]
    pub traffic_jam: bool,
}
