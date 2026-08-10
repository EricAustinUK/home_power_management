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
pub struct RawWeatherData{
    pub hourly: RawHourData
}

#[derive(Clone, Deserialize)]
pub struct HourData {
    pub hour_sin:[f32; 24],
    pub hour_cos:[f32; 24],
    pub shortwave_radiation: [f32; 24],
    pub direct_radiation: [f32; 24],
    pub diffuse_radiation: [f32; 24],
    pub cloud_cover: [f32; 24],
    pub temperature_2m: [f32; 24]
}

#[derive(Clone, Deserialize)]
pub struct RawHourData {
    pub time: [String; 24],
    pub shortwave_radiation: [f32; 24],
    pub direct_radiation: [f32; 24],
    pub diffuse_radiation: [f32; 24],
    pub cloud_cover: [f32; 24],
    pub temperature_2m: [f32; 24]
}

impl TryFrom<RawHourData> for HourData {
    type Error = WeatherDataError;

    fn try_from(raw: RawHourData) -> Result<Self, WeatherDataError> {
        let mut hour_sin:[f32; 24] = [0.; 24];
        let mut hour_cos:[f32; 24] = [0.; 24];

        for i in 0..24 {
            let time = &raw.time[i];
            let hour_str = time.get(11..13).ok_or(WeatherDataError::DateConversion())?;
            let hour:f32 = match hour_str.parse(){
                Ok(h) => h,
                Err(_) => return Err(WeatherDataError::DateConversion())
            };
            let angle:f32 = 2.0 * std::f32::consts::PI * hour / 24.0;

            *hour_sin.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = angle.sin();
            *hour_cos.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = angle.cos();
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