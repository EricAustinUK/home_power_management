pub mod iot_controller;
mod control_panel;
mod ml_engine;
mod weather_data;

pub use iot_controller::IoTError;
use control_panel::PanelState;
use std::sync::{Arc, atomic::AtomicUsize};
use rppal::gpio::{InputPin, Trigger};
use std::time::Duration;
use ml_engine::{MLEngine, MLError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HomeManagerError {
    #[error("Error with IoT layer: {0}")]
    IoTError(#[from] IoTError),
    
    #[error("Error from ML layer: {0}")]
    MLError(#[from] MLError),

    #[error("GPIO error: {0}")]
    GPIO(#[from] rppal::gpio::Error),
}

pub struct HomeManager{
    grid_cap_wh:usize,
    soc_est_wh:usize,
    exp_solar_prod_wh:AtomicUsize,
    exp_house_usg_wh:AtomicUsize,
    control_panel:Arc<PanelState>,
    iot_controller:iot_controller::IoTController,
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
            

            Ok(Self { 
                grid_cap_wh:3840, 
                soc_est_wh:0, 
                exp_solar_prod_wh:AtomicUsize::new(0), 
                exp_house_usg_wh:AtomicUsize::new(6000),
                control_panel:panel,
                iot_controller:iot_controller::IoTController::new()?, 
                tgl_pins,
            })
    }

    pub fn train(&self){

    }

    pub fn predict(&mut self){
        
    }
}