use std::time::{Instant};
use thiserror::Error;
use url::Url;
use dotenvy::dotenv;
use std::env;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("Network error: {0}")]
    Endpoint(#[from] reqwest::Error),
    
    #[error("GPIO error: {0}")]
    GPIO(#[from] rppal::gpio::Error),

    #[error("Error loading .env file (please use .env.template to create a .env file): {0}")]
    MissingEnv(#[from] dotenvy::Error),

    #[error("Missing environment variable '{name}': {err}")]
    MissingEnvVar {
        name: &'static str,
        #[source]
        err: env::VarError,
    },

    #[error("Error parsing value of: '{name}")]
    EnvValueParse {
        name: &'static str,
    },
}

struct IoTConfig {
    hass_host:Url,
    hass_port:u16,
    battery_url:Url,
    ev_url:Url,
    ev_charger_url:Url,
    weather_api_url:Url,
    panel_latitude:f32,
    panel_longitude:f32
}

#[derive(Clone)]
pub struct WeatherData{

}

pub struct IoTController{
    soc_perc:(u8, Instant),
    ev_perc:(u8, Instant),
    weather_data:(WeatherData, Instant),
    iot_config: IoTConfig
}


impl IoTController{
    pub fn new() -> Result<Self, InitError> {
        dotenv()?;
        
        let hass_host:Url = match env::var("HASS_HOST") {
            Ok(url_str) => match Url::parse(&url_str){
                Ok(url) => url,
                Err(e) => return Err( InitError::EnvValueParse { name:"HASS_HOST" } )
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "HASS_HOST", err: e })
        };
        let hass_port:u16 = match env::var("HASS_PORT"){
            Ok(port_str) => match port_str.parse::<u16>(){
                Ok(port) => port,
                Err(e) => return Err(InitError::EnvValueParse { name: "HASS_PORT" })
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "HASS_PORT", err: e })
        };
        let battery_url:Url = match env::var("BATTERY_URL") {
            Ok(url_str) => match Url::parse(&url_str){
                Ok(url) => url,
                Err(e) => return Err(InitError::EnvValueParse { name:"BATTERY_URL" } )
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "BATTERY_URL", err: e })
        };
        let ev_url:Url = match env::var("EV_URL") {
            Ok(url_str) => match Url::parse(&url_str){
                Ok(url) => url,
                Err(e) => return Err(InitError::EnvValueParse { name:"EV_URL" })
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "EV_URL", err: e })
        };
        let ev_charger_url:Url = match env::var("EV_CHARGER_URL") {
            Ok(url_str) => match Url::parse(&url_str){
                Ok(url) => url,
                Err(e) => return Err(InitError::EnvValueParse { name:"EV_CHARGER_URL" })
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "EV_CHARGER_URL", err: e })
        };
        let weather_url:Url = match env::var("WEATHER_API_URL") {
            Ok(url_str) => match Url::parse(&url_str){
                Ok(url) => url,
                Err(e) => return Err(InitError::EnvValueParse { name:"WEATHER_API_URL" })
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "WEATHER_API_URL", err: e })
        };
        let panel_latitude:f32 = match env::var("PANEL_LATITUTDE"){
            Ok(lat_str) => match lat_str.parse::<f32>(){
                Ok(lat) => lat,
                Err(e) => return Err(InitError::EnvValueParse { name: "PANEL_LATITUTDE" })
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "PANEL_LATITUTDE", err: e })
        };
        let panel_longitude:f32 = match env::var("PANEL_LONGITUDE"){
            Ok(lon_str) => match lon_str.parse::<f32>(){
                Ok(lon) => lon,
                Err(e) => return Err(InitError::EnvValueParse { name: "PANEL_LONGITUDE" })
            },
            Err(e) => return Err(InitError::MissingEnvVar { name: "PANEL_LONGITUDE", err: e })
        };

        let cfg = IoTConfig { 
            hass_host:hass_host,
            hass_port:hass_port,
            battery_url:battery_url,
            ev_url:ev_url,
            ev_charger_url:ev_charger_url,
            weather_api_url:weather_url,
            panel_latitude:panel_latitude,
            panel_longitude:panel_longitude
        };

        let soc_perc:(u8, Instant) = IoTController::fetch_soc_perc(&cfg.battery_url)?;
        let ev_perc:(u8, Instant) = IoTController::fetch_ev_perc(&cfg.ev_url)?;
        let weather:(WeatherData, Instant) = IoTController::fetch_weather_data(&cfg.weather_api_url)?;

        let iot_controller  = Self { soc_perc:soc_perc, ev_perc:ev_perc, weather_data:weather, iot_config:cfg };


        Ok(iot_controller)
    }

    pub fn get_ev_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, reqwest::Error>{
        let ret = match min_cache {
            Some(min) => match min < self.ev_perc.1 {
                true => Ok(self.ev_perc.0),
                false => Ok(IoTController::fetch_ev_perc(&self.iot_config.ev_url)?.0)
            },
            None => Ok(IoTController::fetch_ev_perc(&self.iot_config.ev_url)?.0)
        };

        let val = ret?;
        
        self.ev_perc = (val, Instant::now());
        return Ok(val);
    }

    fn get_soc_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, reqwest::Error>{
        let ret = match min_cache {
            Some(min) => match min < self.soc_perc.1 {
                true => Ok(self.soc_perc.0),
                false => Ok(IoTController::fetch_soc_perc(&self.iot_config.battery_url)?.0)
            },
            None => Ok(IoTController::fetch_soc_perc(&self.iot_config.battery_url)?.0)
        };

        let val = ret?;

        self.soc_perc = (val, Instant::now());
        return Ok(val);
    }

    fn get_weather_data(&mut self, min_cache:Option<Instant>) -> Result<&WeatherData, reqwest::Error>{
        let ret = match min_cache {
            Some(min) => match min < self.weather_data.1 {
                true => Ok(self.weather_data.0.clone()),
                false => Ok(IoTController::fetch_weather_data(&self.iot_config.weather_api_url)?.0)
            },
            None => Ok(IoTController::fetch_weather_data(&self.iot_config.weather_api_url)?.0)
        };

        self.weather_data = (ret?, Instant::now());
        return Ok(&self.weather_data.0);
    }

    fn fetch_ev_perc(url:&Url) -> Result<(u8, Instant), reqwest::Error>{
        Ok((67, Instant::now()))
    } 

    fn fetch_soc_perc(url:&Url) -> Result<(u8, Instant), reqwest::Error>{
        Ok((67, Instant::now()))
    }

    fn fetch_weather_data(url:&Url) -> Result<(WeatherData, Instant), reqwest::Error>{
        Ok((WeatherData {}, Instant::now()))
    }

}