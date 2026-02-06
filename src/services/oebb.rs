use chrono::{DateTime, Utc};
use reqwest::Client;

use crate::{
    dtos::internal::{DepartureDto, TripDto},
    models::{
        internal::{IntervalLio, Station},
        oebb::{Departure, Departures, Location},
    },
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

pub async fn fetch_trips_for_lios(
    lios: &Vec<&IntervalLio>,
) -> Result<Vec<TripDto>, Box<dyn std::error::Error>> {
    let ids = lios
        .iter()
        .map(|l| l.provider_id.clone())
        .collect::<Vec<String>>();

    let mut departures: Vec<Departure> = Vec::new();
    for id in ids {
        Client::new()
            .get(format!(
                "https://oebb.macistry.com/api/stops/{}/departures",
                id
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

    Ok(lios
        .iter()
        .map(|lio| (*lio, find_departures_matching_lio(&departures, lio)))
        .map(|pair| lio_departures_pair_to_trip_dto(&pair))
        .collect::<Vec<TripDto>>())
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

fn find_departures_matching_lio<'a>(
    departures: &'a Vec<Departure>,
    lio: &'a IntervalLio,
) -> Vec<&'a Departure> {
    departures
        .iter()
        .filter(|departure| {
            departure
                .line
                .name
                .replace(" ", "")
                .to_lowercase()
                .contains(&lio.line.to_lowercase())
                && departure
                    .direction
                    .to_lowercase()
                    .contains(&lio.direction.to_lowercase())
        })
        .collect::<Vec<&Departure>>()
}

fn lio_departures_pair_to_trip_dto(pair: &(&IntervalLio, Vec<&Departure>)) -> TripDto {
    let (lio, departures) = pair;

    TripDto {
        line: lio.line.clone(),
        direction: lio.direction.clone(),
        foot_minutes_to_station: 5,
        departures: departures
            .iter()
            .map(|d| departure_to_departure_dto(d))
            .collect::<Vec<DepartureDto>>(),
    }
}

fn departure_to_departure_dto(departure: &Departure) -> DepartureDto {
    let calc_countdown = |when: DateTime<Utc>| when.signed_duration_since(Utc::now()).num_minutes();

    let planned_when_date_time = DateTime::parse_from_rfc3339(departure.planned_when.as_str())
        .unwrap()
        .with_timezone(&Utc);

    let (countdown, real_time, late) = departure.when.clone().map_or_else(
        || (calc_countdown(planned_when_date_time), false, false),
        |w| {
            let when_date_time = DateTime::parse_from_rfc3339(w.as_str())
                .unwrap()
                .with_timezone(&Utc);
            (
                calc_countdown(when_date_time),
                true,
                when_date_time > planned_when_date_time,
            )
        },
    );

    DepartureDto {
        direction: Some(departure.direction.clone()),
        countdown: countdown as i32,
        real_time: real_time,
        late: late,
        traffic_jam: false,
    }
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
