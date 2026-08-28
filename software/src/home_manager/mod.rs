pub mod iot_controller;
mod control_panel;
mod ml_engine;
mod weather_data;

pub use iot_controller::IoTError;
use iot_controller::{IoTController, IoTConfig};
use control_panel::{PanelError, PanelState};
use time::{Time, Date, OffsetDateTime, macros::time};
use url::Url;
use std::{str::FromStr, sync::mpsc::{Receiver}};
use std::{env, time::Duration};
use ml_engine::{MLEngine, MLError};
use thiserror::Error;
use dotenvy::dotenv;
use ureq::{http::Uri};

use crate::home_manager::weather_data::WeatherData;

const TARIFF_START:Time = time!(23:30);
const TARIFF_END:Time = time!(5:30);

#[derive(Debug, Error)]
pub enum HomeManagerError {
    #[error("Error with IoT layer: {0}")]
    IoT(#[from] IoTError),
    
    #[error("Error from ML layer: {0}")]
    ML(#[from] MLError),

    #[error("GPIO error: {0}")]
    Panel(#[from] PanelError),

    #[error("Error importing .env: {0}")]
    DotEnv(#[from] DotEnvError),
}

#[derive(Clone, Copy)]
pub enum State{
    Offline,
    Online(TimeState)
}

#[derive(Clone, Copy)]
pub enum TimeState{
    Standard,
    Tariff
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

#[derive(Clone)]
struct HomeManagerData{
    state:State,
    exp_solar_prod_wh:[f64; 24],
    exp_house_usg_wh:f64,
    home_soc:f32,
    ev_soc:f32,
    home_soc_min:f32,
    home_soc_max:f32
}

pub struct HomeManager{
    control_panel:PanelState,
    iot_controller:IoTController,
    ml_engine:MLEngine,
    data:HomeManagerData,
    gpio_rx:Receiver<u8>
}

impl HomeManager{
    pub fn new() -> Result<Self, HomeManagerError> {
        let (tx, rx) = std::sync::mpsc::channel();
        let env = Self::load_env()?;
        
        Ok(Self {
            data:HomeManagerData {
                state:State::Offline,
                exp_solar_prod_wh:[0.; 24],
                exp_house_usg_wh:3600.,
                ev_soc:0.,
                home_soc:0.,
                home_soc_min:23.,
                home_soc_max:95.
            },
            control_panel:PanelState::new(&tx)?,
            iot_controller:IoTController::new(env.iot_cfg)?,
            ml_engine:MLEngine::new(env.model_bytes, env.model_data_path)?,
            gpio_rx:rx
        })
    }

    pub fn state_loop(&mut self, new_state:&TimeState, date_str:String, time_str:String) -> Result<(), HomeManagerError>{
        self.run_state_action(new_state)?;
        self.control_panel.update(&self.data, date_str, time_str);

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), HomeManagerError>{
        loop{
            let now = time::OffsetDateTime::now_local().map_err(|e| HomeManagerError::IoT(IoTError::LocalTimeError(e)))?;
            let date = now.date();
            let time = now.time();
            let date_str = format!("{:02}/{:02}/{}", date.day(), date.month() as u8, (date.year() % 100).abs());
            let time_str = format!("{:02}:{:02}", time.hour(), time.minute());

            match self.gpio_rx.recv_timeout(Duration::from_secs(5)){
                Ok(pin) => {
                    self.handle_pin(pin);
                    self.control_panel.update(&self.data, date_str, time_str);
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let now = OffsetDateTime::now_local().map_err(|e| HomeManagerError::IoT(IoTError::LocalTimeError(e)))?;
                    let new_state = if now.time() > TARIFF_END && now.time() < TARIFF_START {TimeState::Standard} else {TimeState::Tariff};
                    self.state_loop(&new_state, date_str, time_str)?;
                },
                Err(e) => {
                    return Err(HomeManagerError::Panel(PanelError::Recv(e)));
                }
            }
        }
    }

    fn run_state_action(&mut self, new_state:&TimeState) -> Result<(), HomeManagerError>{
        match (self.data.state, new_state) {
            (State::Offline, ts) => {
                if self.iot_controller.test_endpoints()?{
                    match *ts {
                        TimeState::Tariff => if !self.try_tariff_start()? {self.data.state = State::Offline; return Ok(())},
                        TimeState::Standard => return Ok(())
                    }
                }else{
                    return Ok(()) // early return if offline
                }
            },
            (State::Online(TimeState::Tariff), TimeState::Standard) => {
                if !self.try_tariff_end()? {
                    self.data.state = State::Offline;
                    return Ok(())
                }
            },
            (State::Online(TimeState::Tariff), TimeState::Tariff) => {
                if !self.try_tariff_update()? {
                    self.data.state = State::Offline;
                    return Ok(())
                }
            },
            (State::Online(TimeState::Standard), TimeState::Tariff) => {
                if !self.try_tariff_start()? {
                    self.data.state = State::Offline;
                    return Ok(())
                }
            },
            (State::Online(TimeState::Standard), TimeState::Standard) => ()
        }
        self.update_ev_charger()?;
        self.data.state =  State::Online(*new_state);
        Ok(())
    }

    fn train(&mut self, real_solar_data:&[f64; 24], real_weather_data:&WeatherData) -> Result<(), HomeManagerError>{
        Ok(self.ml_engine.train(real_weather_data, real_solar_data)?)
    }

    fn predict(&mut self, date:Date) -> Result<[f64; 24], HomeManagerError>{
        let data = self.iot_controller.fetch_weather_data(date)?;
        Ok(self.ml_engine.infer(&data)?)
    }

    pub fn handle_pin(&mut self, pin:u8){
        self.control_panel.handle_pin(pin);
    }

    fn try_tariff_start(&mut self) -> Result<bool, HomeManagerError>{
        match self.tariff_start() {
            Ok(()) => Ok(true),
            Err(HomeManagerError::IoT(IoTError::Endpoint(_))) => {
                self.data.state = State::Offline;
                Ok(false)
            },
            Err(e) => Err(e)
        }
    }

    fn try_tariff_end(&mut self) -> Result<bool, HomeManagerError>{
        match self.tariff_end() {
            Ok(()) => Ok(true),
            Err(HomeManagerError::IoT(IoTError::Endpoint(_))) => {
                self.data.state = State::Offline;
                Ok(false)
            },
            Err(e) => Err(e)
        }
    }

    fn try_tariff_update(&mut self) -> Result<bool, HomeManagerError>{
        match self.tariff_update() {
            Ok(()) => Ok(true),
            Err(HomeManagerError::IoT(IoTError::Endpoint(_))) => {
                self.data.state = State::Offline;
                Ok(false)
            },
            Err(e) => Err(e)
        }
    }

    fn tariff_start(&mut self) -> Result<(), HomeManagerError>{
        // get previous date
        let current_time = OffsetDateTime::now_local().map_err(|e| IoTError::from(e))?;
        let jic_offset = current_time - Duration::from_hours(6);
        let y_date = jic_offset.date();
        
        let real_solar_data = self.iot_controller.fetch_hourly_solar_output_wh(y_date)?;
        let (real_total, est_total): (f64, f64) = self.data.exp_solar_prod_wh
            .iter()
            .zip(real_solar_data.iter())
            .fold((0., 0.), |(acc_est, acc_real), (est, real)| {
                (acc_est + est, acc_real + real)
            });

        println!("Actual solar output deviated by {}Wh from the expected total.", (real_total - est_total) as i32);

        // Train step

        let real_weather_data = self.iot_controller.fetch_weather_data(y_date)?;

        self.train(&real_solar_data, &real_weather_data)?;

        // Inference step

        let t_date = y_date.next_day().unwrap(); // safe to unwrap until the heat death of the universe
        let exp_solar_out = self.predict(t_date)?;
        self.data.exp_solar_prod_wh = exp_solar_out;
        
        println!("Expecting solar output to be {}Wh.",  self.data.exp_solar_prod_wh.iter().fold(0., |acc, n| acc + n));

        // Adjustment step

        self.update_soc_target()?;

        Ok(())
    }

    fn tariff_end(&mut self) -> Result<(), HomeManagerError>{
        self.iot_controller.set_min_soc(23)?;
        Ok(())
    }

    fn tariff_update(&mut self) -> Result<(), HomeManagerError>{
        let current_time = OffsetDateTime::now_local().map_err(|e| IoTError::from(e))?;
        let jic_offset = current_time + Duration::from_hours(6);
        let t_date = jic_offset.date();
        
        // Inference step
        let exp_solar_out = self.predict(t_date)?;
        self.data.exp_solar_prod_wh = exp_solar_out;
        
        // Adjustment step
        self.update_soc_target()?;

        Ok(())
    }

    fn update_soc_target(&mut self) -> Result<(), IoTError>{
        let est_usage:f64  = self.control_panel.get_usage_est(self.data.exp_house_usg_wh);
        let total_solar:f64 = self.data.exp_solar_prod_wh.iter().fold(0., |acc, n| acc + n);
        
        let need_wh = est_usage - total_solar;
        if need_wh <= 0.{
            self.iot_controller.set_min_soc(self.data.home_soc_min as u8)
        }else{
            let need_perc_raw = (need_wh * 100. / 1920.).min((self.data.home_soc_max-self.data.home_soc_min) as f64);
            let need_perc = need_perc_raw.round() as u8;
            self.iot_controller.set_min_soc(self.data.home_soc_min as u8 + need_perc)
        }
    }

    pub fn update_ev_charger(&self) -> Result<(), IoTError>{
        let target_perc = self.control_panel.ev_target_perc;
        match self.iot_controller.fetch_ev_info()?{
            (soc, true, true) => {
                if soc >= target_perc {
                    return self.iot_controller.set_ev_charger(false)
                }
            },
            (_,_,_) => ()
        };
        self.iot_controller.set_ev_charger(true)
    }

    fn load_env() -> Result<HomeManagerEnv, DotEnvError>{
        dotenv()?;
        
        let port:u16 = match env::var("HASS_PORT"){
            Ok(port_str) => match port_str.parse::<u16>(){
                Ok(port) => port,
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "HASS_PORT" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "HASS_PORT", err: e })
        };
        let hass_host:Url = match env::var("HASS_IP") {
            Ok(url_str) => match Url::from_str(&format!("http://{url_str}:{port}")){
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
        let ev_name:String = match env::var("EV_NAME") {
            Ok(name_str) => name_str,
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "EV_NAME", err: e })
        };
        let ev_charger_name:String = match env::var("EV_CHARGER_NAME") {
            Ok(name_str) => name_str,
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "EV_CHARGER_NAME", err: e })
        };
        let weather_url:Uri = match env::var("WEATHER_API_URL") {
            Ok(url_str) => match Uri::from_str(&url_str){
                Ok(url) => url,
                Err(_) => return Err(DotEnvError::EnvValueParse { name:"WEATHER_API_URL" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "WEATHER_API_URL", err: e })
        };
        let panel_latitude:f32 = match env::var("PANEL_LATITUDE"){
            Ok(lat_str) => match lat_str.parse::<f32>(){
                Ok(lat) => lat,
                Err(_) => return Err(DotEnvError::EnvValueParse { name: "PANEL_LATITUDE" })
            },
            Err(e) => return Err(DotEnvError::MissingEnvVar { name: "PANEL_LATITUDE", err: e })
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
                hass_token:hass_token,
                battery_name:battery_name,
                ev_name:ev_name,
                ev_charger_name:ev_charger_name,
                weather_api_url:weather_url,
                panel_latitude:panel_latitude,
                panel_longitude:panel_longitude
            },
            model_bytes:model_bytes,
            model_data_path:model_data_filename
        })
    }
}