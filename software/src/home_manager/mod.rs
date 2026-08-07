pub mod iot_controller;
mod control_panel;

pub use iot_controller::WeatherData;
use control_panel::PanelState;
use std::sync::{Arc, atomic::AtomicUsize};
use rppal::gpio::{InputPin, Trigger};
use std::time::Duration;

use crate::home_manager::iot_controller::IoTController;

pub struct HomeManager{
    grid_cap_wh:usize,
    soc_est_wh:usize,
    exp_solar_prod_wh:AtomicUsize,
    exp_house_usg_wh:AtomicUsize,
    control_panel:Arc<PanelState>,
    iot_controller:IoTController,
    tgl_pins:Vec<InputPin>,
}

impl HomeManager{
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let panel = Arc::new(PanelState::new()?);

    let tgl_pins: Vec<InputPin> = [11, 12, 13]
        .into_iter()
        .map(|pin_no| {
            let mut pin = panel.gpio.get(pin_no)?.into_input_pullup();
            let pin_no = pin.pin();
            
            // Clone the Arc to move into the closure safely
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
            iot_controller:IoTController::new()?, 
            tgl_pins,
        })
    }
}