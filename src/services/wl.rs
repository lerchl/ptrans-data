use csv::ReaderBuilder;
use reqwest::Client;

use crate::{
    dtos::internal::{DepartureDto, TripDto},
    models::{
        internal::{IntervalLio, Station},
        wl::{Line, Monitor, MonitorResponse, StationCsvRow},
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

pub async fn fetch_monitors(divas: Vec<String>) -> Result<MonitorResponse, reqwest::Error> {
    let divas_param = divas.join(",");
    let url = format!(
        "https://www.wienerlinien.at/ogd_realtime/monitor?diva={}",
        divas_param
    );

    let resp = Client::new()
        .get(url)
        .send()
        .await?
        .json::<MonitorResponse>()
        .await?;

    Ok(resp)
}

pub async fn fetch_trips_for_lios(
    lios: &Vec<&IntervalLio>,
) -> Result<Vec<TripDto>, Box<dyn std::error::Error>> {
    let divas = lios
        .iter()
        .map(|l| l.provider_id.clone())
        .collect::<Vec<String>>()
        .join(",");

    let url = format!(
        "https://www.wienerlinien.at/ogd_realtime/monitor?diva={}",
        divas
    );

    let monitor_response = Client::new()
        .get(url)
        .send()
        .await?
        .json::<MonitorResponse>()
        .await?;

    let lines = monitor_response
        .data
        .monitors
        .iter()
        .filter_map(|m| m.lines.first())
        .collect::<Vec<&Line>>();

    Ok(lios
        .iter()
        .map(|lio| (*lio, find_line_matching_lio(&lines, lio)))
        .map(|pair| lio_line_pair_to_trip(&pair))
        .collect::<Vec<TripDto>>())
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

fn lio_line_pair_to_trip(pair: &(&IntervalLio, Option<&Line>)) -> TripDto {
    let (lio, line) = pair;

    TripDto {
        line: lio.line.clone(),
        direction: lio.direction.clone(),
        foot_minutes_to_station: 5,
        departures: line.map_or(vec![], |l| {
            l.departures
                .departure
                .iter()
                .map(|d| DepartureDto {
                    direction: d.vehicle.towards.clone(),
                    when: d.departure_time.time_planned.clone(),
                    when_actually: d.departure_time.time_real.clone(),
                    traffic_jam: d.vehicle.trafficjam,
                })
                .collect::<Vec<DepartureDto>>()
        }),
    }
}

// fn map_line_to_lio<'a>(
//     lios: &'a Vec<&IntervalLio>,
//     line: &'a Line,
// ) -> Option<(&'a IntervalLio, &'a Line)> {
//     lios.iter()
//         .filter(|l| {
//             line.name
//                 .trim()
//                 .to_lowercase()
//                 .contains(&l.line.to_lowercase())
//                 && line
//                     .towards
//                     .trim()
//                     .to_lowercase()
//                     .contains(&l.direction.to_lowercase())
//         })
//         .map(|l| (*l, line))
//         .next()
// }

fn lios_target_line(lios: &Vec<&IntervalLio>, line: &Line) -> bool {
    lios.iter().any(|l| {
        line.name
            .trim()
            .to_lowercase()
            .contains(&l.line.to_lowercase())
            && line
                .towards
                .trim()
                .to_lowercase()
                .contains(&l.direction.to_lowercase())
    })
}

pub fn filter_monitors_for_lios(monitors: &Vec<Monitor>, lios: &Vec<&IntervalLio>) -> Vec<Monitor> {
    let is_line_targeted = |line: &Line| lios_target_line(lios, line);

    monitors
        .iter()
        .filter(|m| m.lines.first().is_some_and(is_line_targeted))
        .cloned()
        .collect::<Vec<Monitor>>()
}

pub fn format_monitors_plain(monitors: &Vec<Monitor>) -> Vec<String> {
    monitors
        .iter()
        .map(|m| {
            format!(
                "{:3} -> {:20} in {:3} minutes",
                m.lines.first().unwrap().name.trim(),
                m.lines.first().unwrap().towards.trim(),
                m.lines.first().unwrap().departures.departure[0]
                    .departure_time
                    .countdown,
            )
        })
        .collect::<Vec<String>>()
}
