use std::{sync::mpsc::Sender, time::Duration};
use rppal::gpio::{Gpio, InputPin, Level, OutputPin, Trigger};
use thiserror::Error;

mod display;
use display::{Display, DisplayError};

#[derive(Error, Debug)]
pub enum PanelError {
    #[error("Error with GPIO pins: {0}")]
    GPIO(#[from] rppal::gpio::Error),
    
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

        let mut display = Display::new()?;
        
        display.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
        display.update();
        
        println!("UPDATED DISPLAY");

        Ok(Self { app_1:false, app_2:false, app_3:false, lan:false, leds:leds, ev_target_perc:95, _tgl_pins:tgl_pins,  display:display })
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

}
