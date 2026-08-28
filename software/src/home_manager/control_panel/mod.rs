use std::{sync::mpsc::{RecvTimeoutError, Sender}, time::Duration};
use rppal::gpio::{Gpio, InputPin, Level, OutputPin, Trigger};
use thiserror::Error;

mod display;
use display::{Display, DisplayError, DisplayData};

use crate::home_manager::{HomeManagerData, State, TimeState::Tariff};

#[derive(Error, Debug)]
pub enum PanelError {
    #[error("Error with GPIO pins: {0}")]
    GPIO(#[from] rppal::gpio::Error),
    
    #[error("Error with GPIO interrupt receiver: {0}")]
    Recv(#[from] RecvTimeoutError),

    #[error("Error connecting slint to display: {0}")]
    Display(#[from] DisplayError),
}


pub struct PanelState {
    pub app_1: bool,
    pub app_2: bool,
    pub app_3: bool,
    lan: bool, 
    leds: [OutputPin; 4],
    pub ev_target_perc:u8,
    _tgl_pins: Vec<InputPin>,
    display:Display
}

impl PanelState{
    pub fn new (tx:&Sender<u8>) -> Result<Self, PanelError>{
        let gpio = Gpio::new()?;
        let leds = Self::init_leds(&gpio)?;
        
        let tgl_pins: Vec<InputPin> = [17, 18, 27, 20, 21]
            .into_iter()
            .map(|pin_no| {
                let mut pin = gpio.get(pin_no)?.into_input_pullup();
                let pin_no = pin.pin();
                let btn_tx = tx.clone();

                pin.set_async_interrupt(
                    Trigger::RisingEdge,
                    Some(Duration::from_millis(50)),
                    move |_| {
                        match btn_tx.send(pin_no){
                            Ok(_) => (),
                            Err(e) => println!("Error sending GPIO signal to channel for event: {:?}", e) // TODO: add retry
                        }
                    },
                )?;

                Ok(pin) 
            })
            .collect::<Result<Vec<InputPin>, rppal::gpio::Error>>()?;

        Ok(Self { app_1:false, app_2:false, app_3:false, lan:false, leds:leds, ev_target_perc:95, _tgl_pins:tgl_pins,  display:Display::new()? })
    }

    pub fn update(&mut self, data:&HomeManagerData, date_str:String, time_str:String){
        self.display.update(
            DisplayData {
                date_str:date_str,
                time_str:time_str,
                solar_est_wh:data.exp_solar_prod_wh.iter().sum(),
                usage_est_wh:self.get_usage_est(data.exp_house_usg_wh),
                home_soc_percent:data.home_soc,
                ev_soc_percent:data.ev_soc,
                ev_soc_target:self.ev_target_perc,
                home_soc_min:data.home_soc_min as u8,
                home_soc_max:data.home_soc_max as u8,
                tariff:match data.state { State::Online(Tariff) => true, _ => false }, // wont be displayed anyway if false
                online:match data.state { State::Online(_) => true, State::Offline => false }
            }
        )
    }

    pub fn handle_pin(&mut self, pin:u8){
        let (on, ind) = match pin {
            17 => (&mut self.app_1, 0),
            18 => (&mut self.app_2, 1),
            27 => (&mut self.app_3, 2),
            // ev_charge
            20 => { self.ev_target_perc = (self.ev_target_perc + 5).min(95); return },
            21 => { self.ev_target_perc = (self.ev_target_perc - 5).max(25); return },
            _ => return,
        };

        *on = !*on;

        self.leds[ind].write(if *on {Level::High} else {Level::Low});
    }

    pub fn set_lan(&mut self, on:bool){
        self.lan = on;
        self.leds[3].write(if on {Level::High} else {Level::Low});
    }

    pub fn init_leds(gpio:&Gpio) -> Result<[OutputPin; 4], rppal::gpio::Error> {
        let led_app1 = gpio.get(23)?.into_output();
        let led_app2 = gpio.get(24) ?.into_output();
        let led_app3 = gpio.get(25)?.into_output();
        let led_lan = gpio.get(22)?.into_output();

        Ok([led_app1, led_app2, led_app3, led_lan])
    }

    pub fn get_usage_est(&self, base_load:f64) -> f64{
        // todo: make app appliance pvs configurable
        base_load + if self.app_1 {2400.} else {0.} + if self.app_2 {0.} else {0.} + if self.app_3 {0.} else {0.}
    }

}
