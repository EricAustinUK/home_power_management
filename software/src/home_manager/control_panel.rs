use std::{sync::mpsc::{Receiver, Sender}, time::Duration};

use rppal::gpio::{Event, Gpio, InputPin, Level, OutputPin, Trigger};

pub struct PanelState {
    app_1: bool,
    app_2: bool,
    app_3: bool,
    leds: [OutputPin; 3],
    _tgl_pins: Vec<InputPin>
}

impl PanelState{
    pub fn new (tx:&Sender<u8>) -> Result<Self, rppal::gpio::Error>{
        let gpio = Gpio::new()?;
        let leds = Self::init_leds(&gpio)?;
        
        let tgl_pins: Vec<InputPin> = [11, 12, 13]
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

        
        Ok(Self { app_1:false, app_2:false, app_3:false, leds:leds, _tgl_pins:tgl_pins })
    }

    pub fn toggle(&mut self, pin:u8){
        let (app, ind) = match pin {
            11 => (&mut self.app_1, 0),
            12 => (&mut self.app_2, 1),
            13 => (&mut self.app_3, 2),
            _ => return,
        };

        *app = !*app;

        self.leds[ind].write(if *app {Level::High} else {Level::Low});
    }

    pub fn init_leds(gpio:&Gpio) -> Result<[OutputPin; 3], rppal::gpio::Error> {
        let led1 = gpio.get(16)?.into_output();
        let led2 = gpio.get(18)?.into_output();
        let led3 = gpio.get(22)?.into_output();

        Ok([led1, led2, led3])
    }

}
