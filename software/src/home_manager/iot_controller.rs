use std::time::{Instant};
use url::Url;


pub const BATTERY_URL: &str = "https://api.example.com/";
pub const EV_URL: &str = "https://api.example.com/";
pub const EV_CHARGER_URL: &str = "https://api.example.com/";
pub const WEATHER_API_URL: &str = "https://api.example.com/";

#[derive(Clone)]
pub struct WeatherData{

}

pub struct IoTController{
    soc_perc:(u8, Instant),
    ev_perc:(u8, Instant),
    weather_data:(WeatherData, Instant)
}


impl IoTController{
    pub fn new() -> Result<Self, reqwest::Error> {
        let soc_perc:(u8, Instant) = IoTController::fetch_soc_perc()?;

        let ev_perc:(u8, Instant) = IoTController::fetch_ev_perc()?;

        let weather:(WeatherData, Instant) = IoTController::fetch_weather_data()?;

        Ok(Self { soc_perc:soc_perc, ev_perc:ev_perc, weather_data:weather })
    }

    pub fn get_ev_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, reqwest::Error>{
        let ret = match min_cache {
            Some(min) => match min < self.ev_perc.1 {
                true => Ok(self.ev_perc.0),
                false => Ok(IoTController::fetch_ev_perc()?.0)
            },
            None => Ok(IoTController::fetch_ev_perc()?.0)
        };

        let val = ret?;
        
        self.ev_perc = (val, Instant::now());
        return Ok(val);
    }

    fn get_soc_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, reqwest::Error>{
        let ret = match min_cache {
            Some(min) => match min < self.soc_perc.1 {
                true => Ok(self.soc_perc.0),
                false => Ok(IoTController::fetch_soc_perc()?.0)
            },
            None => Ok(IoTController::fetch_soc_perc()?.0)
        };

        let val = ret?;

        self.soc_perc = (val, Instant::now());
        return Ok(val);
    }

    fn get_weather_data(&mut self, min_cache:Option<Instant>) -> Result<&WeatherData, reqwest::Error>{
        let ret = match min_cache {
            Some(min) => match min < self.weather_data.1 {
                true => Ok(self.weather_data.0.clone()),
                false => Ok(IoTController::fetch_weather_data()?.0)
            },
            None => Ok(IoTController::fetch_weather_data()?.0)
        };

        self.weather_data = (ret?, Instant::now());
        return Ok(&self.weather_data.0);
    }

    fn fetch_ev_perc() -> Result<(u8, Instant), reqwest::Error>{
        Ok((67, Instant::now()))
    } 

    fn fetch_soc_perc() -> Result<(u8, Instant), reqwest::Error>{
        Ok((67, Instant::now()))
    }

    fn fetch_weather_data() -> Result<(WeatherData, Instant), reqwest::Error>{
        Ok((WeatherData {}, Instant::now()))
    }

}