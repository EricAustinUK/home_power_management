use std::{str::FromStr, time::{Instant, SystemTime}};
use serde::Deserialize;
use thiserror::Error;
use ureq::{Agent, http::{Uri, response}};
use time::{Date, Duration, OffsetDateTime, PlainDateTime, Time, UtcDateTime, UtcOffset, format_description::well_known::Rfc3339 };
use url::Url;
pub use crate::home_manager::weather_data::{WeatherData, WeatherDataError};

#[derive(Debug, Error)]
pub enum IoTError {
    #[error("Network error: {0}")]
    Endpoint(#[from] ureq::Error),

    #[error("Error parsing value in Weather API")]
    WeatherAPIError(#[from] WeatherDataError),

    #[error("ureq considers URL invalid")]
    InvalidURL(),

    #[error("Pi cannot determine the local time offset")]
    LocalTimeError(#[from] time::error::IndeterminateOffset),

    #[error("Could not parse date/time recieved from endpoint")]
    DateConversionError(#[from] time::error::Parse),

    #[error("Could not parse float recieved from endpoint")]
    FloatConversionError(#[from] std::num::ParseFloatError),

    #[error("Home assistant returned empty array for history")]
    InvalidHassResponse(),
}

pub struct IoTConfig {
    pub hass_host:Url,
    pub hass_token:String,
    pub battery_name:String,
    pub ev_name:String,
    pub ev_charger_name:String,
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

#[derive(Deserialize)]
pub struct PvDataSample{
    pub state:String,
    pub last_updated:String
}

pub struct EvData{
    pub state:String,
    pub last_updated:String
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

    pub fn get_soc_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, ureq::Error>{
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

        let weather_data:WeatherData = response.into_body().read_json()?;

        println!("Successfully retrieved {} hours of weather data.", weather_data.hourly.time.iter().count());

        Ok((weather_data, Instant::now()))
    }

    pub fn fetch_date_weather_data(&self, date:Date) -> Result<WeatherData, IoTError>{

        let response = self.ureq_agent
        .get(&self.iot_config.weather_api_url)
        .query("latitude", self.iot_config.panel_latitude.to_string())
        .query("longitude", self.iot_config.panel_longitude.to_string())
        .query("hourly", "shortwave_radiation,direct_radiation,diffuse_radiation,temperature_2m,cloud_cover")
        .query("tilt", "30")
        .query("azimuth", "0")
        .query("start_date", date.to_string())
        .query("end_date", date.to_string())
        .query("timezone", "Europe/London")
        .call()?;

        let weather_data:WeatherData = response.into_body().read_json()?;

        println!("Successfully received {} hours of historic weather data for training:", weather_data.hourly.time.iter().count());

        Ok(weather_data)
    }

    pub fn fetch_hourly_solar_output_wh(&self, date:Date) -> Result<[f64; 24], IoTError>{
        let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);

        let start_time = date.with_time(Time::from_hms(5, 0, 0).unwrap());
        let start_dt = start_time.assume_offset(local_offset);

        let start: String = start_dt.format(&Rfc3339).unwrap();


        let end_time = date.with_time(Time::from_hms(21, 0, 0).unwrap());
        let end_dt = end_time.assume_offset(local_offset);

        let end: String = end_dt.format(&Rfc3339).unwrap();


        let mut history_endpoint = self.iot_config.hass_host.clone();
        history_endpoint.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("history").push("period").push(&start);

        let uri:Uri = Uri::from_str(&history_endpoint.as_str()).map_err(|_| IoTError::InvalidURL())?;

        let result = self.ureq_agent.get(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .query("end_time", &end)
        .query("filter_entity_id", &format!("sensor.{}_power_pv_sum", &self.iot_config.battery_name))
        .call()?;

        let pv_data_outer:Vec<Vec<PvDataSample>> = result.into_body().read_json()?;

        let pv_data = match pv_data_outer.first(){
            Some(pv_data) => pv_data,
            None => return Err(IoTError::InvalidHassResponse())
        };
        let mut prev_t:OffsetDateTime = start_dt;
        let mut this_hour:usize = 5;

        let mut response_arr:[f64;24] = [0.; 24];

        for sample in pv_data {
            let pv:f64 = sample.state.parse()?;
            let sample_t = PlainDateTime::parse(&sample.last_updated, &Rfc3339)?.assume_offset(local_offset);
            let t_s = sample_t.unix_timestamp() - prev_t.unix_timestamp();
            let t_h:f64 = t_s as f64 / 3600.;

            if sample_t.hour() as usize == this_hour{
                response_arr[this_hour] += pv * t_h;
            }else if (sample_t.hour() as usize == this_hour + 1){
                let split_s =  sample_t.date().with_hms(sample_t.hour(), 0, 0).unwrap().assume_offset(local_offset).unix_timestamp() - prev_t.unix_timestamp();
                response_arr[this_hour] += pv * (split_s as f64 / t_s as f64)/3600.;
                response_arr[this_hour+1] = pv * ((t_s - split_s) as f64 / t_s as f64)/3600.;
                
                this_hour += 1;
            }else{
                // throw some kind of error for being an hour gap in pv
            }
            prev_t = sample_t;
        }

        Ok(response_arr)
    }

}