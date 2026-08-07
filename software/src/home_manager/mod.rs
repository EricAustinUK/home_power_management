pub mod iot_controller;

pub use iot_controller::WeatherData;
use std::{sync::{atomic::{AtomicUsize}}};

pub struct HomeManager{
    grid_cap_wh:usize,
    soc_est_wh:usize, 
    exp_solar_prod_wh:AtomicUsize,
    exp_house_usg_wh:AtomicUsize,
}

impl HomeManager{
    pub const fn new() -> Result<Self, reqwest::Error> {
        Ok(Self { grid_cap_wh:3840, soc_est_wh:0, exp_solar_prod_wh:AtomicUsize::new(0) , exp_house_usg_wh:AtomicUsize::new(6000) })
    }
}