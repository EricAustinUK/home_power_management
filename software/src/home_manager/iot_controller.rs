use std::time::Instant;
use thiserror::Error;
use ureq::{Agent, http::Uri};
use time::{Duration};
pub use crate::home_manager::weather_data::{WeatherData, RawWeatherData, WeatherDataError};

#[derive(Debug, Error)]
pub enum IoTError {
    #[error("Network error: {0}")]
    Endpoint(#[from] ureq::Error),

    #[error("Error parsing value in Weather API")]
    WeatherAPIError(#[from] WeatherDataError)
}

pub struct IoTConfig {
    pub hass_host:Uri,
    pub hass_port:u16,
    pub battery_url:Uri,
    pub ev_url:Uri,
    pub ev_charger_url:Uri,
    pub weather_api_url:Uri,
    pub panel_latitude:f32,
    pub panel_longitude:f32
}


pub struct IoTController{
    soc_perc:(u8, Instant),
    ev_perc:(u8, Instant),
    weather_data:(WeatherData, Instant),
    ureq_agent:Agent,
    iot_config: IoTConfig
}


impl IoTController{
    pub fn new(cfg:IoTConfig) -> Result<Self, IoTError> {
        

        let agent = Agent::new_with_defaults();

        let soc_perc:(u8, Instant) = IoTController::fetch_soc_perc(&agent, &cfg)?;
        let ev_perc:(u8, Instant) = IoTController::fetch_ev_perc(&agent, &cfg)?;
        let weather:(WeatherData, Instant) = IoTController::fetch_weather_data(&agent, &cfg)?;

        let iot_controller  = Self { soc_perc:soc_perc, ev_perc:ev_perc, weather_data:weather, ureq_agent:agent, iot_config:cfg };


        Ok(iot_controller)
    }

    pub fn get_ev_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, ureq::Error>{
        let val = match min_cache {
            Some(min) => match min < self.ev_perc.1 {
                true => self.ev_perc.0,
                false => IoTController::fetch_ev_perc( &self.ureq_agent, &self.iot_config)?.0
            },
            None => IoTController::fetch_ev_perc( &self.ureq_agent, &self.iot_config)?.0
        };
        
        self.ev_perc = (val, Instant::now());
        return Ok(val);
    }

    fn get_soc_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, ureq::Error>{
        let val = match min_cache {
            Some(min) => match min < self.soc_perc.1 {
                true => self.soc_perc.0,
                false => IoTController::fetch_soc_perc( &self.ureq_agent, &self.iot_config)?.0
            },
            None => IoTController::fetch_soc_perc( &self.ureq_agent, &self.iot_config)?.0
        };

        self.soc_perc = (val, Instant::now());
        return Ok(val);
    }

    fn get_weather_data(&mut self, min_cache:Option<Instant>) -> Result<&WeatherData, IoTError>{
        match min_cache {
            Some(min) => match min < self.soc_perc.1 {
                true => Ok(&self.weather_data.0),
                false => { 
                    let new_wd = IoTController::fetch_weather_data( &self.ureq_agent, &self.iot_config)?;
                    self.weather_data = new_wd;
                    Ok(&self.weather_data.0)
                }
            },
            None => { 
                let new_wd = IoTController::fetch_weather_data( &self.ureq_agent, &self.iot_config)?;
                self.weather_data = new_wd;
                Ok(&self.weather_data.0)
            }
        }
    }

    fn fetch_ev_perc(agent:&Agent, cfg:&IoTConfig) -> Result<(u8, Instant), ureq::Error>{
        Ok((67, Instant::now()))
    } 

    fn fetch_soc_perc(agent:&Agent, cfg:&IoTConfig) -> Result<(u8, Instant), ureq::Error>{
        Ok((67, Instant::now()))
    }

    fn fetch_weather_data(agent:&Agent, cfg:&IoTConfig) -> Result<(WeatherData, Instant), IoTError>{
        let tomorrow = (time::OffsetDateTime::now_utc().to_offset(time::macros::offset!(+1)) + Duration::days(1)).date().to_string();

        let response = agent
        .get(&cfg.weather_api_url)
        .query("latitude", cfg.panel_latitude.to_string())
        .query("longitude", cfg.panel_longitude.to_string())
        .query("hourly", "shortwave_radiation,direct_radiation,diffuse_radiation,temperature_2m,cloud_cover")
        .query("tilt", "30")
        .query("azimuth", "0")
        .query("start_date", &tomorrow)
        .query("end_date", &tomorrow)
        .query("timezone", "Europe/London")
        .call()?;

        let raw_weather_data:RawWeatherData = response.into_body().read_json()?;

        println!("Successfully retrieved {} hours of weather data.", raw_weather_data.hourly.time.iter().count());

        Ok((WeatherData {hourly:raw_weather_data.hourly.try_into()? }, Instant::now()))
    }

    pub fn fetch_prev_weather_data(&self) -> Result<WeatherData, IoTError>{
        let today = time::OffsetDateTime::now_utc().to_offset(time::macros::offset!(+1)).date().to_string();

        let response = self.ureq_agent
        .get(&self.iot_config.weather_api_url)
        .query("latitude", self.iot_config.panel_latitude.to_string())
        .query("longitude", self.iot_config.panel_longitude.to_string())
        .query("hourly", "shortwave_radiation,direct_radiation,diffuse_radiation,temperature_2m,cloud_cover")
        .query("tilt", "30")
        .query("azimuth", "0")
        .query("start_date", &today)
        .query("end_date", &today)
        .query("timezone", "Europe/London")
        .call()?;

        let raw_data:RawWeatherData = response.into_body().read_json()?;

        println!("Successfully received {} hours of historic weather data for training:", raw_data.hourly.time.iter().count());

        Ok(WeatherData { hourly:raw_data.hourly.try_into()? })
    }

}