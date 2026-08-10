use serde::Deserialize;
use thiserror::Error;
use crate::home_manager::ml_engine::WeatherFeatures;

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

impl TryInto<WeatherFeatures> for WeatherData {
    type Error = WeatherDataError;
    fn try_into(self) -> Result<WeatherFeatures, Self::Error> {
        let feature_arrs = self.hourly;
        
        let mut hour_sin = [0.0; 24];
        let mut hour_cos = [0.0; 24];

        for i in 0..24 {
            let time = &feature_arrs.time[i];
            let hour_str = time
                .get(11..13)
                .ok_or(WeatherDataError::DateConversion())?;

            let hour: f32 = hour_str
                .parse()
                .map_err(|_| WeatherDataError::DateConversion())?;

            let angle = 2.0 * std::f32::consts::PI * hour / 24.0;

            hour_sin[i] = angle.sin();
            hour_cos[i] = angle.cos();
        }

        let mut result = WeatherFeatures {
            features: [[0.0; 7]; 24],
        };

        for i in 0..24 {
            result.features[i] = [
                hour_sin[i] as f64,
                hour_cos[i] as f64,
                feature_arrs.shortwave_radiation[i] as f64,
                feature_arrs.direct_radiation[i] as f64,
                feature_arrs.diffuse_radiation[i] as f64,
                feature_arrs.cloud_cover[i] as f64,
                feature_arrs.temperature_2m[i] as f64,
            ];
        }

        Ok(result)
    }
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