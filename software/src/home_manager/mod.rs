pub mod iot_controller;
mod control_panel;
mod ml_engine;
mod weather_data;

pub use iot_controller::IoTError;
use iot_controller::{IoTController, IoTConfig};
use control_panel::PanelState;
use std::{str::FromStr, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Instant};
use rppal::gpio::{InputPin, Trigger};
use std::{env, time::Duration};
use ml_engine::{MLEngine, MLError};
use thiserror::Error;
use dotenvy::dotenv;
use ureq::http::Uri;

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
    control_panel:Arc<PanelState>,
    iot_controller:IoTController,
    tgl_pins:Vec<InputPin>,
    ml_engine:MLEngine
}

impl HomeManager{
    pub fn new() -> Result<Self, HomeManagerError> {
        let panel = Arc::new(PanelState::new()?);

        let tgl_pins: Vec<InputPin> = [11, 12, 13]
            .into_iter()
            .map(|pin_no| {
                let mut pin = panel.gpio.get(pin_no)?.into_input_pullup();
                let pin_no = pin.pin();
                
                let panel_clone = Arc::clone(&panel);

                pin.set_async_interrupt(
                    Trigger::RisingEdge,
                    Some(Duration::from_millis(50)),
                    move |_| {
                        panel_clone.toggle(pin_no);
                        println!("Rising edge detected on pin {}.", pin_no);
                    },
                )?;

                Ok(pin) 
            })
            .collect::<Result<Vec<InputPin>, rppal::gpio::Error>>()?;

            let iot_cfg = Self::load_env()?;

            Ok(Self { 
                grid_cap_wh:3840, 
                soc_est_wh:0, 
                exp_solar_prod_wh:std::array::from_fn(|_| AtomicUsize::new(0)), // TEMP: CHANGE THIS DISGUSTING AI FIX ASAP
                real_solar_prod_wh:std::array::from_fn(|_| AtomicUsize::new(0)), // TEMP: CHANGE THIS DISGUSTING AI FIX ASAP
                exp_house_usg_wh:AtomicUsize::new(6000),
                control_panel:panel,
                iot_controller:IoTController::new(iot_cfg)?, 
                tgl_pins:tgl_pins,
                ml_engine:MLEngine::new()?
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

    fn load_env() -> Result<IoTConfig, DotEnvError>{
        dotenv()?;
        
        let hass_host:Uri = match env::var("HASS_HOST") {
            Ok(url_str) => match Uri::from_str(&url_str){
                Ok(url) => url,
                Err(_) => return Err( DotEnvError::EnvValueParse { name:"HASS_HOST" } )
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "HASS_HOST", err: e })
        };
        let hass_port:u16 = match env::var("HASS_PORT"){
            Ok(port_str) => match port_str.parse::<u16>(){
                Ok(port) => port,
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "HASS_PORT" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "HASS_PORT", err: e })
        };
        let battery_url:Uri = match env::var("BATTERY_URL") {
            Ok(url_str) => match Uri::from_str(&url_str){
                Ok(url) => url,
                Err(_) => return Err(DotEnvError::EnvValueParse { name:"BATTERY_URL" } )
            },
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

        Ok(IoTConfig { 
            hass_host:hass_host,
            hass_port:hass_port,
            battery_url:battery_url,
            ev_url:ev_url,
            ev_charger_url:ev_charger_url,
            weather_api_url:weather_url,
            panel_latitude:panel_latitude,
            panel_longitude:panel_longitude
        })
    }
}