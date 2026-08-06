use rppal::gpio::{Gpio, InputPin, OutputPin, Trigger};
use std::{sync::{Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};
use std::time::{Instant, Duration};
use either::{Either, Left, Right};
use url::Url;

pub const BATTERY_URL: &str = "https://api.example.com/";
pub const EV_URL: &str = "https://api.example.com/";
pub const EV_CHARGER_URL: &str = "https://api.example.com/";
pub const WEATHER_API_URL: &str = "https://api.example.com/";

struct PanelState {
    app_1: AtomicBool,
    app_2: AtomicBool,
    app_3: AtomicBool,
    leds: Mutex<Option<[OutputPin; 3]>>,
}

struct HomeManager{
    grid_cap_wh:usize,
    soc_est_wh:usize, 
    exp_solar_prod_wh:AtomicUsize,
    exp_house_usg_wh:AtomicUsize,
}

#[derive(Clone)]
struct WeatherData{

}

struct IoTController{
    soc_perc:(u8, Instant),
    ev_perc:(u8, Instant),
    weather_data:(WeatherData, Instant)
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

impl HomeManager{
    const fn new() -> Result<Self, reqwest::Error> {
        Ok(Self { grid_cap_wh:3840, soc_est_wh:0, exp_solar_prod_wh:AtomicUsize::new(0) , exp_house_usg_wh:AtomicUsize::new(6000) })
    }
}

impl IoTController{
    fn new() -> Result<Self, Either<url::ParseError, reqwest::Error>> {
        let soc_perc:(u8, Instant) = match IoTController::fetch_soc_perc() {
            Ok(pc) => pc,
            Err(e) => return Err(Right(e))
        };

        let ev_perc:(u8, Instant) = match IoTController::fetch_ev_perc() {
            Ok(pc) => pc,
            Err(e) => return Err(Right(e))
        };

        let weather:(WeatherData, Instant) = match IoTController::fetch_weather_data() {
            Ok(data) => data,
            Err(e) => return Err(Right(e))
        };

        Ok(Self { soc_perc:soc_perc, ev_perc:ev_perc, weather_data:weather })
    }

    fn get_ev_perc(&mut self, min_cache:Option<Instant>) -> Result<u8, reqwest::Error>{
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