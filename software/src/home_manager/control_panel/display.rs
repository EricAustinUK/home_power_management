use rppal::gpio::{Gpio, OutputPin};
use rppal::spi::{Bus, Mode, SlaveSelect, Spi, SimpleHalSpiDevice};
use rppal::hal::Delay;
use st7735_lcd::ST7735;
use st7735_lcd::Orientation;
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::Platform;
use std::rc::Rc;
use std::time::Instant;
use thiserror::Error;

slint::include_modules!();

#[derive(Error, Debug)]
pub enum DisplayError{
    #[error("Error with Graphics layer: {0}")]
    Graphics(#[from] slint::PlatformError),

    #[error("Error setting platform: {0}")]
    SetPlatform(#[from] slint::platform::SetPlatformError),

    #[error("Error with GPIO: {0}")]
    GPIO(#[from] rppal::gpio::Error),

    #[error("Error with SPI: {0}")]
    SPI(#[from] rppal::spi::Error),

    #[error("Error initialising LCD driver")]
    Driver()
}

pub struct Display {
    pub ui: App,
    pub _driver: ST7735<SimpleHalSpiDevice, OutputPin, OutputPin>,
    window: Rc<MinimalSoftwareWindow>,
    buffer: Vec<slint::platform::software_renderer::Rgb565Pixel>,
}

pub struct DisplayData {
    pub date_str:String,
    pub time_str:String,
    pub solar_est_wh:f64,
    pub usage_est_wh: f64,
    pub home_soc_percent:f32,
    pub ev_soc_percent:f32,
    pub ev_soc_target:u8,
    pub home_soc_min:u8,
    pub home_soc_max:u8,
    pub tariff:bool,
    pub online:bool,
}

 struct PiDisplayPlatform {
    window: Rc<MinimalSoftwareWindow>,
    start_time: Instant,
}


impl Platform for PiDisplayPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    } 
}

impl Display {
    pub fn new() -> Result<Self, DisplayError> {
        let gpio = Gpio::new()?;

        let dc = gpio.get(26)?.into_output();
        let rst = gpio.get(19)?.into_output();

        let spi = Spi::new(
            Bus::Spi0,
            SlaveSelect::Ss0,
            4_000_000,
            Mode::Mode0,
        )?;

        let spi_device = SimpleHalSpiDevice::new(spi);

        let mut delay = Delay::new();
        let mut disp = ST7735::new(spi_device, dc, rst, true, false, 128, 160);

        disp.init(&mut delay).map_err(|_| DisplayError::Driver())?;
        disp.set_orientation(&Orientation::Portrait).map_err(|_| DisplayError::Driver())?;
        disp.set_offset(2, 1);
        
        let window = MinimalSoftwareWindow::new(slint::platform::software_renderer::RepaintBufferType::ReusedBuffer);
        window.set_size(slint::PhysicalSize::new(128, 160));

        let platform = PiDisplayPlatform {
            window: window.clone(),
            start_time: Instant::now(),
        };
        
        slint::platform::set_platform(Box::new(platform))?;

        let ui = App::new()?;
        
        let buffer = vec![slint::platform::software_renderer::Rgb565Pixel(0); 128 * 160];
        
        Ok(Self { ui, _driver: disp, window, buffer })
    }

    pub fn update(&mut self, display_data:DisplayData) {
        self.ui.set_date_str(display_data.date_str.into());
        self.ui.set_time_str(display_data.time_str.into());
        self.ui.set_solar_est_wh(display_data.solar_est_wh as i32);
        self.ui.set_usage_est_wh(display_data.usage_est_wh as i32);
        self.ui.set_home_soc_percent(display_data.home_soc_percent);
        self.ui.set_ev_soc_percent(display_data.ev_soc_percent);
        self.ui.set_ev_soc_target(display_data.ev_soc_target as i32);
        self.ui.set_home_soc_min(display_data.home_soc_min as i32);
        self.ui.set_home_soc_max(display_data.home_soc_max as i32);
        self.ui.set_tariff(display_data.tariff);
        self.ui.set_online(display_data.online);
        

        slint::platform::update_timers_and_animations();
        self.window.request_redraw(); 

        self.window.draw_if_needed(|renderer| {
            renderer.render(&mut self.buffer, 128);

            let colors = self.buffer.iter().map(|p| p.0);
            let _ = self._driver.set_pixels(0, 0, 127, 159, colors);
        });
    }
}