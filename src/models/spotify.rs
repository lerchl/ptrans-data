use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PlayerResponse {
    pub is_playing: bool,
    pub item: Option<TrackItem>,
    pub device: DeviceInfo,
}

#[derive(Deserialize, Debug)]
pub struct DeviceInfo {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct TrackItem {
    pub album: Album,
}

#[derive(Deserialize, Debug)]
pub struct Album {
    pub images: Vec<Image>,
}

#[derive(Deserialize, Debug)]
pub struct Image {
    pub url: String,
    pub width: Option<u32>,
}
