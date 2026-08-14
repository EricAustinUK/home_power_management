use std::{str::FromStr};
use serde::Deserialize;
use thiserror::Error;
use ureq::{Agent, http::{Uri}};
use time::{Date, Duration, OffsetDateTime, PlainDateTime, Time, UtcOffset, format_description::well_known::Rfc3339 };
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

    #[error("Could not parse integer recieved from endpoint")]
    IntConversionError(#[from] std::num::ParseIntError),

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
    ureq_agent:Agent,
    iot_config: IoTConfig
}

#[derive(Deserialize)]
pub struct PvDataSample{
    pub state:String,
    pub last_updated:String
}

#[derive(Deserialize)]
pub struct HassSample{
    pub state:String,
    pub last_updated:String
}

impl IoTController{
    pub fn new(cfg:IoTConfig) -> Result<Self, IoTError> {
        let agent = Agent::new_with_defaults();

        
        let iot_controller  = Self { ureq_agent:agent, iot_config:cfg };

        // Check endpoints
        let tomorrow = (time::OffsetDateTime::now_local()? + Duration::days(1)).date();
        iot_controller.fetch_soc_perc()?;
        iot_controller.fetch_ev_info()?;
        iot_controller.fetch_weather_data(tomorrow)?;

        Ok(iot_controller)
    }

    pub fn fetch_ev_info(&self) -> Result<(u8, bool, bool), IoTError>{
        let mut url = self.iot_config.hass_host.clone();
        url.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("states").push(&format!("sensor.{}_battery_level", &self.iot_config.ev_name));
        let uri = url.to_string();

        let result = self.ureq_agent.get(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .call()?;

        let perc_data:HassSample = result.into_body().read_json()?;
        let ev_perc:f32 = perc_data.state.parse()?;
        
        // possibly add check for data staleness?

        let mut url = self.iot_config.hass_host.clone();
        url.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("states").push(&format!("device_tracker.{}_location", &self.iot_config.ev_name));
        let uri = url.to_string();

        let result = self.ureq_agent.get(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .call()?;

        let charge_state_data:HassSample = result.into_body().read_json()?;

        // possibly add check for data staleness?

        let mut url = self.iot_config.hass_host.clone();
        url.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("states").push(&format!("sensor.{}_plug", &self.iot_config.ev_name));
        let uri = url.to_string();

        let result = self.ureq_agent.get(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .call()?;

        let plug_state_data:HassSample = result.into_body().read_json()?;

        // possibly add check for data staleness?

        Ok((ev_perc as u8, charge_state_data.state == "home", plug_state_data.state == "on"))
    }

    fn fetch_soc_perc(&self) -> Result<f32, IoTError>{
        let mut url = self.iot_config.hass_host.clone();
        url.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("states").push(&format!("sensor.{}_power_battery_soc",self.iot_config.battery_name));
        let uri = url.to_string();

        let result = self.ureq_agent.get(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .call()?;

        let data:HassSample = result.into_body().read_json()?;
        let soc:f32 = data.state.parse()?;
        
        // possibly add check for data staleness?

        Ok(soc)
    }


    pub fn fetch_weather_data(&self, date:Date) -> Result<WeatherData, IoTError>{
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


    pub fn set_min_soc(&self, perc:u8) -> Result<(), IoTError>{
        let mut url = self.iot_config.hass_host.clone();
        url.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("services").push("number").push("set_value");
        let uri = url.to_string();

        let body = format!(r#"{{
            "entity_id": "number.{}",
            "value": {}
        }}"#, self.iot_config.battery_name, perc);

        self.ureq_agent.post(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .send(body)?;

        Ok(())
    }

    pub fn set_ev_charger(&self, state:bool) -> Result<(), IoTError>{
        let mut url = self.iot_config.hass_host.clone();
        url.path_segments_mut().map_err(|_| IoTError::InvalidURL())?.push("api").push("services").push("switch").push(if state {"turn_on"} else { "turn_off" } );
        let uri = url.to_string();

        let body = format!(r#"{{
            "entity_id": "switch.{}"
        }}"#, self.iot_config.ev_charger_name);

        self.ureq_agent.post(&uri)
        .header("Authorization",&format!("Bearer {}", self.iot_config.hass_token))
        .header("Content-Type", "application/json")
        .send(body)?;

        Ok(())
    }

}