use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StationCsvRow {
    #[serde(rename = "DIVA")]
    pub diva: String,
    #[serde(rename = "PlatformText")]
    pub platform_text: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub data: Data,
    // pub message: Message,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub monitors: Vec<Monitor>,
}

// #[derive(Debug, Deserialize)]
// pub struct Message {
//     pub value: String,
//     pub messageCode: i32,
//     pub serverTime: String,
// }

#[derive(Debug, Deserialize)]
pub struct Monitor {
    // pub locationStop: LocationStop,
    pub lines: Vec<Line>,
}

// #[derive(Debug, Deserialize)]
// pub struct LocationStop {
//     #[serde(rename = "type")]
//     pub feature_type: String,
//     pub geometry: Geometry,
//     pub properties: StopProperties,
// }

// #[derive(Debug, Deserialize)]
// pub struct Geometry {
//     #[serde(rename = "type")]
//     pub geometry_type: String,
//     pub coordinates: [f64; 2], // [longitude, latitude]
// }

// #[derive(Debug, Deserialize)]
// pub struct StopProperties {
//     pub name: String,
//     pub title: String,
//     pub municipality: String,
//     pub municipalityId: i64,
//     #[serde(rename = "type")]
//     pub stop_type: String,
//     pub coordName: String,
//     pub gate: String,
//     pub attributes: StopAttributes,
// }

// #[derive(Debug, Deserialize)]
// pub struct StopAttributes {
//     pub rbl: i32,
// }

#[derive(Debug, Deserialize)]
pub struct Line {
    pub name: String,
    pub towards: String,
    #[serde(rename = "realtimeSupported")]
    pub realtime_supported: bool,
    pub trafficjam: bool,
    pub departures: Departures,
}

#[derive(Debug, Deserialize)]
pub struct Departures {
    pub departure: Vec<Departure>,
}

#[derive(Debug, Deserialize)]
pub struct Departure {
    #[serde(rename = "departureTime")]
    pub departure_time: DepartureTime,
    pub vehicle: Option<Vehicle>,
}

#[derive(Debug, Deserialize)]
pub struct DepartureTime {
    #[serde(rename = "timePlanned")]
    pub time_planned: String,
    #[serde(rename = "timeReal")]
    pub time_real: String,
    pub countdown: i32,
}

#[derive(Debug, Deserialize)]
pub struct Vehicle {
    pub name: String,
    pub towards: String,
    #[serde(rename = "realtimeSupported")]
    pub realtime_supported: bool,
    pub trafficjam: bool,
}
