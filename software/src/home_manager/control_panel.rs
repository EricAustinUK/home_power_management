use rppal::gpio::{Gpio, OutputPin};
use std::{sync::{Mutex, atomic::{AtomicBool, Ordering}}};

pub struct PanelState {
    app_1: AtomicBool,
    app_2: AtomicBool,
    app_3: AtomicBool,
    leds: Mutex<Option<[OutputPin; 3]>>,
    pub gpio: Gpio,
}

impl PanelState{
    pub fn new () -> Result<Self, rppal::gpio::Error>{
        let panel:PanelState = Self { app_1: AtomicBool::new(false), app_2: AtomicBool::new(false), app_3: AtomicBool::new(false), leds: Mutex::new(None), gpio:Gpio::new()? };
        panel.init_leds()?;
        Ok(panel)
    }

    pub fn toggle(&self, pin:u8){
        let app = match pin {
            11 => &self.app_1,
            12 => &self.app_2,
            13 => &self.app_3,
            _ => return,
        };

        app.fetch_not(Ordering::SeqCst);

        if let Ok(mut guard) = self.leds.lock() {
            if let Some(pins) = guard.as_mut() {
                pins[0].write(self.app_1.load(Ordering::SeqCst).into());
                pins[1].write(self.app_2.load(Ordering::SeqCst).into());
                pins[2].write(self.app_3.load(Ordering::SeqCst).into());
            }
        }
    }

    pub fn init_leds(&self) -> Result<(), rppal::gpio::Error> {
        let mut led1 = self.gpio.get(16)?.into_output();
        let mut led2 = self.gpio.get(18)?.into_output();
        let mut led3 = self.gpio.get(22)?.into_output();

        led1.write(self.app_1.load(Ordering::SeqCst).into());
        led2.write(self.app_2.load(Ordering::SeqCst).into());
        led3.write(self.app_3.load(Ordering::SeqCst).into());

        if let Ok(mut guard) = self.leds.lock() {
            *guard = Some([led1, led2, led3]);
        }

        Ok(())
    }

}
