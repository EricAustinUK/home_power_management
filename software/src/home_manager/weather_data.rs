use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WeatherDataError {
    #[error("Error converting date from weather API data")]
    DateConversion(),

    #[error("Error converting floats from weather API data")]
    FloatConversion(),

    #[error("Weather API returned an unexpected number of records")]
    DataOverflow()
}

#[derive(Clone, Deserialize)]
pub struct WeatherData{
    pub hourly: HourData
}

#[derive(Clone, Deserialize)]
pub struct HourData {
    pub time: [String; 24],
    pub shortwave_radiation: [f32; 24],
    pub direct_radiation: [f32; 24],
    pub diffuse_radiation: [f32; 24],
    pub cloud_cover: [f32; 24],
    pub temperature_2m: [f32; 24]
}