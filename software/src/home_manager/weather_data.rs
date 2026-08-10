use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WeatherDataError {
    #[error("Error converting date from weather API data")]
    DateConversion(),

    #[error("Error converting floats from weather API data")]
    FloatConversion(),
}

#[derive(Clone, Deserialize)]
pub struct WeatherData{
    pub hourly: HourData
}

#[derive(Clone, Deserialize)]
pub struct RawWeatherData{
    pub hourly: RawHourData
}

#[derive(Clone, Deserialize)]
pub struct HourData {
    pub hour_sin:Vec<f32>,
    pub hour_cos:Vec<f32>,
    pub shortwave_radiation: Vec<f32>,
    pub direct_radiation: Vec<f32>,
    pub diffuse_radiation: Vec<f32>,
    pub cloud_cover: Vec<f32>,
    pub temperature_2m: Vec<f32>
}

#[derive(Clone, Deserialize)]
pub struct RawHourData {
    pub time: Vec<String>,
    pub shortwave_radiation: Vec<f32>,
    pub direct_radiation: Vec<f32>,
    pub diffuse_radiation: Vec<f32>,
    pub cloud_cover: Vec<f32>,
    pub temperature_2m: Vec<f32>
}

impl TryFrom<RawHourData> for HourData {
    type Error = WeatherDataError;

    fn try_from(raw: RawHourData) -> Result<Self, WeatherDataError> {
        let mut hour_sin = Vec::with_capacity(raw.time.len());
        let mut hour_cos = Vec::with_capacity(raw.time.len());

        for time in &raw.time {
            let hour_str = match time.get(11..13){
                Some(h) => h,
                None => return Err(WeatherDataError::DateConversion())
            };
            let hour:f32 = match hour_str.parse(){
                Ok(h) => h,
                Err(_) => return Err(WeatherDataError::DateConversion())
            };
            let angle:f32 = 2.0 * std::f32::consts::PI * hour / 24.0;

            hour_sin.push(angle.sin());
            hour_cos.push(angle.cos());
        }

        Ok(HourData {
            hour_sin,
            hour_cos,
            shortwave_radiation: raw.shortwave_radiation,
            direct_radiation: raw.direct_radiation,
            diffuse_radiation: raw.diffuse_radiation,
            cloud_cover: raw.cloud_cover,
            temperature_2m: raw.temperature_2m,
        })
    }
}