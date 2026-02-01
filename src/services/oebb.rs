use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Client;

use crate::models::{
    internal::{IntervalLio, Station},
    oebb::{Departure, Departures, Location},
};

pub async fn fetch_stations(name: String) -> Result<Vec<Station>, reqwest::Error> {
    let resp = Client::new()
        .get(format!(
            "https://oebb.macistry.com/api/locations?query={}",
            name
        ))
        .send()
        .await?
        .json::<Vec<Location>>()
        .await?;

    Ok(resp
        .iter()
        .filter(|l| l.kind == "stop")
        .map(|l| Station {
            id: l.id.to_string(),
            provider: "OEBB".to_string(),
            name: l.name.to_string(),
        })
        .collect::<Vec<Station>>())
}

pub async fn fetch_depatures_for_stations(
    ids: Vec<String>,
) -> Result<Vec<Departure>, reqwest::Error> {
    let mut departures: Vec<Departure> = Vec::new();

    for ele in ids {
        Client::new()
            .get(format!(
                "https://oebb.macistry.com/api/stops/{}/departures",
                ele
            ))
            .send()
            .await?
            .json::<Departures>()
            .await?
            .departures
            .iter()
            .for_each(|d| {
                departures.push(d.clone());
            });
    }

    Ok(departures)
}

pub fn filter_departures_for_lios(
    departures: &Vec<Departure>,
    lios: &Vec<&IntervalLio>,
) -> Vec<Departure> {
    departures
        .iter()
        .filter(|d| {
            lios.iter().any(|l| {
                d.line
                    .name
                    .replace(" ", "")
                    .to_lowercase()
                    .contains(&l.line.to_lowercase())
                    && d.direction
                        .to_lowercase()
                        .contains(&l.direction.to_lowercase())
            })
        })
        .cloned()
        .collect::<Vec<Departure>>()
}

pub fn format_departures_plain(departures: &Vec<Departure>) -> Vec<String> {
    departures
        .iter()
        .map(|d| {
            format!(
                "{:3} -> {:20} in {:3} minutes",
                Regex::new(r"\s*\(.*?\)").unwrap().replace(d.line.name.trim(), ""),
                d.direction.trim(),
                DateTime::parse_from_rfc3339(d.when.as_str())
                    .unwrap()
                    .with_timezone(&Utc)
                    .signed_duration_since(Utc::now())
                    .num_minutes()
                    .to_string()
            )
        })
        .collect::<Vec<String>>()
}
