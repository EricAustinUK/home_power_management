pub mod iot_controller;
mod control_panel;
mod ml_engine;
mod weather_data;

pub use iot_controller::IoTError;
use iot_controller::{IoTController, IoTConfig};
use control_panel::PanelState;
use url::Url;
use std::{str::FromStr, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{Receiver, SendError, Sender}}, time::Instant};
use rppal::gpio::{Event, InputPin, Trigger};
use std::{env, time::Duration};
use ml_engine::{MLEngine, MLError};
use thiserror::Error;
use dotenvy::dotenv;
use ureq::{get, http::Uri};

#[derive(Debug, Error)]
pub enum HomeManagerError {
    #[error("Error with IoT layer: {0}")]
    IoTError(#[from] IoTError),
    
    #[error("Error from ML layer: {0}")]
    MLError(#[from] MLError),

    #[error("GPIO error: {0}")]
    GPIO(#[from] rppal::gpio::Error),

    #[error("Error importing .env: {0}")]
    DotEnv(#[from] DotEnvError),
}

struct HomeManagerEnv{
    pub iot_cfg:IoTConfig,
    pub model_bytes:Option<Vec<u8>>,
    pub model_data_path:Option<String>
}


#[derive(Debug, Error)]
pub enum DotEnvError {
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

pub struct HomeManager{
    grid_cap_wh:usize,
    soc_est_wh:usize,
    exp_solar_prod_wh:[AtomicUsize; 24],
    real_solar_prod_wh:[AtomicUsize; 24],
    exp_house_usg_wh:AtomicUsize,
    control_panel:PanelState,
    iot_controller:IoTController,
    ml_engine:MLEngine,
    pub gpio_rx:Receiver<u8>
}

impl HomeManager{
    pub fn new() -> Result<Self, HomeManagerError> {
        let (tx, rx) = std::sync::mpsc::channel();
        let env = Self::load_env()?;
        
        Ok(Self { 
            grid_cap_wh:3840, 
            soc_est_wh:0, 
            exp_solar_prod_wh:std::array::from_fn(|_| AtomicUsize::new(0)), // TEMP: CHANGE THIS DISGUSTING AI FIX ASAP
            real_solar_prod_wh:std::array::from_fn(|_| AtomicUsize::new(0)), // TEMP: CHANGE THIS DISGUSTING AI FIX ASAP
            exp_house_usg_wh:AtomicUsize::new(6000),
            control_panel:PanelState::new(&tx)?,
            iot_controller:IoTController::new(env.iot_cfg)?,
            ml_engine:MLEngine::new(env.model_bytes, env.model_data_path)?,
            gpio_rx:rx
        })
    }

    pub fn train(&mut self) -> Result<(), HomeManagerError>{
        let real_weather_data = self.iot_controller.fetch_prev_weather_data()?;
        let real_solar_data: [f64; 24] = std::array::from_fn(|i| {
            self.real_solar_prod_wh[i].load(Ordering::Relaxed) as f64
        });
        Ok(self.ml_engine.train(real_weather_data, real_solar_data)?)
    }

    pub fn predict(&mut self) -> Result<(), HomeManagerError>{
        let data = self.iot_controller.get_weather_data(Some(Instant::now() - Duration::from_mins(30)))?;
        let predicted = self.ml_engine.infer(data.clone())?;
        
        for (hour_val, pred_val) in self.exp_solar_prod_wh.iter().zip(predicted.iter()) {
            hour_val.store(*pred_val as usize, Ordering::Relaxed);
        }

        Ok(())
    }

    pub fn tgl_pin(&mut self, pin:u8){
        self.control_panel.toggle(pin);
    }

    fn load_env() -> Result<HomeManagerEnv, DotEnvError>{
        dotenv()?;
        
        let hass_port:u16 = match env::var("HASS_PORT"){
            Ok(port_str) => match port_str.parse::<u16>(){
                Ok(port) => port,
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "HASS_PORT" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "HASS_PORT", err: e })
        };
        
        let hass_host:Url = match env::var("HASS_IP") {
            Ok(url_str) => match Url::from_str(&format!("http://{url_str}:{hass_port}")){
                Ok(url) => url,
                Err(_) => return Err( DotEnvError::EnvValueParse { name:"HASS_IP" } )
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "HASS_IP", err: e })
        };

        let hass_token:String = match env::var("HASS_TOKEN"){
            Ok(token) => token,
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "HASS_TOKEN", err: e })
        };
        let battery_name:String = match env::var("BATTERY_NAME") {
            Ok(name_str) => name_str,
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "BATTERY_URL", err: e })
        };
        let ev_url:Uri = match env::var("EV_URL") {
            Ok(url_str) => match Uri::from_str(&url_str){
                Ok(url) => url,
                Err(_) => return Err(DotEnvError::EnvValueParse { name:"EV_URL" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "EV_URL", err: e })
        };
        let ev_charger_url:Uri = match env::var("EV_CHARGER_URL") {
            Ok(url_str) => match Uri::from_str(&url_str){
                Ok(url) => url,
                Err(_) => return Err(DotEnvError::EnvValueParse { name:"EV_CHARGER_URL" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "EV_CHARGER_URL", err: e })
        };
        let weather_url:Uri = match env::var("WEATHER_API_URL") {
            Ok(url_str) => match Uri::from_str(&url_str){
                Ok(url) => url,
                Err(_) => return Err(DotEnvError::EnvValueParse { name:"WEATHER_API_URL" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "WEATHER_API_URL", err: e })
        };
        let panel_latitude:f32 = match env::var("PANEL_LATITUTDE"){
            Ok(lat_str) => match lat_str.parse::<f32>(){
                Ok(lat) => lat,
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "PANEL_LATITUTDE" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "PANEL_LATITUTDE", err: e })
        };
        let panel_longitude:f32 = match env::var("PANEL_LONGITUDE"){
            Ok(lon_str) => match lon_str.parse::<f32>(){
                Ok(lon) => lon,
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "PANEL_LONGITUDE" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "PANEL_LONGITUDE", err: e })
        };

        let model_bytes:Option<Vec<u8>> = match env::var("MODEL_FILENAME"){
            Ok(path_str) => match std::fs::read(path_str){
                Ok(bytes) => Some(bytes),
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "MODEL_FILENAME" })
            },
            Err(_) => None
        };
        
        let model_data_filename:Option<String> = match env::var("MODEL_DATA_FILENAME"){
            Ok(path_str) => match std::fs::exists(&path_str){
                Ok(_) => Some(path_str),
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "MODEL_DATA_FILENAME" })
            },
            Err(_) => None
        };        

        Ok(HomeManagerEnv{ 
            iot_cfg: IoTConfig { 
                hass_host:hass_host,
                hass_port:hass_port,
                hass_token:hass_token,
                battery_name:battery_name,
                ev_url:ev_url,
                ev_charger_url:ev_charger_url,
                weather_api_url:weather_url,
                panel_latitude:panel_latitude,
                panel_longitude:panel_longitude
            },
            model_bytes:model_bytes,
            model_data_path:model_data_filename
        })
    }
}