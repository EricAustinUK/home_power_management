use rppal::gpio::{Gpio, InputPin, OutputPin, Trigger};
use std::{sync::atomic::AtomicBool, sync::atomic::Ordering, sync::Mutex, time::Duration};

struct PanelState {
    app_1: AtomicBool,
    app_2: AtomicBool,
    app_3: AtomicBool,
    leds: Mutex<Option<[OutputPin; 3]>>,
}

impl PanelState{
    const fn new () -> Self{
        Self { app_1: AtomicBool::new(false), app_2: AtomicBool::new(false), app_3: AtomicBool::new(false), leds: Mutex::new(None), }
    }

    fn toggle(&self, pin:u8){
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

    fn init_leds(&self, gpio: &Gpio) -> Result<(), rppal::gpio::Error> {
        let mut led1 = gpio.get(16)?.into_output();
        let mut led2 = gpio.get(18)?.into_output();
        let mut led3 = gpio.get(22)?.into_output();

        led1.write(self.app_1.load(Ordering::SeqCst).into());
        led2.write(self.app_2.load(Ordering::SeqCst).into());
        led3.write(self.app_3.load(Ordering::SeqCst).into());

        if let Ok(mut guard) = self.leds.lock() {
            *guard = Some([led1, led2, led3]);
        }

        Ok(())
    }

}

static CONTROL_PANEL: PanelState = PanelState::new();

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let gpio = Gpio::new()?; 
    CONTROL_PANEL.init_leds(&gpio)?;

    let tgl_pins: Vec<InputPin> = [11, 12, 13]
    .into_iter()
    .map(|pin_no| {
        let mut pin = gpio.get(pin_no).unwrap().into_input_pullup();
        let pin_no = pin.pin();

        pin.set_async_interrupt(Trigger::RisingEdge,
            Some(Duration::from_millis(50)),
            move |_| {
            CONTROL_PANEL.toggle(pin_no);
            println!("Rising edge detected on pin {}.", pin_no);
        })
        .unwrap();

        pin // Return pin to store in the Vec
    })
    .collect();

    std::thread::park();

    Ok(())
}