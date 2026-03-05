use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use itertools::Itertools;
use reqwest::Client;

use crate::{
    dtos::internal::{DepartureDto, TripDto},
    models::{
        internal::{IntervalLio, Station},
        wl::{Data, Departure, Line, MonitorResponse, StationCsvRow},
    },
};

pub async fn get_stations() -> Result<Vec<Station>, Box<dyn std::error::Error>> {
    let resp = Client::new()
        .get("https://www.wienerlinien.at/ogd_realtime/doku/ogd/wienerlinien-ogd-haltestellen.csv")
        .send()
        .await?
        .text()
        .await?;

    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .from_reader(resp.as_bytes());

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

pub async fn fetch_trips_for_lios(
    lios: &Vec<&IntervalLio>,
) -> Vec<TripDto> {
    let divas = lios
        .iter()
        .map(|l| l.provider_id.clone())
        .unique()
        .collect::<Vec<String>>()
        .join(",");

    let url = format!(
        "https://www.wienerlinien.at/ogd_realtime/monitor?diva={}",
        divas
    );

    let monitor_response = match Client::new().get(url).send().await {
        Ok(response) => response
            .json::<MonitorResponse>()
            .await
            .unwrap_or_else(|_| MonitorResponse {
                data: Data { monitors: vec![] },
            }),
        Err(_) => MonitorResponse {
            data: Data { monitors: vec![] },
        },
    };

    let lines = monitor_response
        .data
        .monitors
        .iter()
        .filter_map(|m| m.lines.first())
        .collect::<Vec<&Line>>();

    lios.iter()
        .map(|lio| (*lio, find_line_matching_lio(&lines, lio)))
        .map(|pair| lio_line_pair_to_trip_dto(&pair))
        .collect::<Vec<TripDto>>()
}

fn find_line_matching_lio<'a>(lines: &'a Vec<&Line>, lio: &'a IntervalLio) -> Option<&'a Line> {
    lines
        .iter()
        .filter(|line| {
            line.name
                .trim()
                .to_lowercase()
                .contains(&lio.line.to_lowercase())
                && line
                    .towards
                    .trim()
                    .to_lowercase()
                    .contains(&lio.direction.to_lowercase())
        })
        .next()
        .map(|line| *line)
}

fn lio_line_pair_to_trip_dto(pair: &(&IntervalLio, Option<&Line>)) -> TripDto {
    let (lio, line) = pair;

    TripDto {
        line: lio.line.clone(),
        direction: lio.direction.clone(),
        foot_minutes_to_station: 5,
        departures: line.map_or(vec![], |l| {
            l.departures
                .departure
                .iter()
                .map(|d| line_departure_to_departure_dto(d))
                .collect::<Vec<DepartureDto>>()
        }),
    }
}

fn line_departure_to_departure_dto(d: &Departure) -> DepartureDto {
    let real_time = d
        .clone()
        .vehicle
        .map(|v| v.realtime_supported)
        .unwrap_or(false);

    let late = if !real_time {
        false
    } else {
        d.clone()
            .departure_time
            .time_real
            .map(|tr| {
                let time_real = tr.parse::<DateTime<Utc>>().unwrap();
                let time_planned = d
                    .departure_time
                    .time_planned
                    .parse::<DateTime<Utc>>()
                    .unwrap();

                time_real > time_planned
            })
            .unwrap_or(false)
    };

    DepartureDto {
        direction: d.clone().vehicle.map(|v| v.towards.trim().to_string()),
        countdown: d.departure_time.countdown,
        real_time: real_time,
        late: late,
        traffic_jam: d.clone().vehicle.map(|v| v.traffic_jam).unwrap_or(false),
    }
}
