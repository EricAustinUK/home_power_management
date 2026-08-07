mod home_manager;

use std::{sync::{Mutex, atomic::{AtomicBool, Ordering}}};

use home_manager::{HomeManager, WeatherData};

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let home_manager = HomeManager::new()?;

    std::thread::park();

    Ok(())
}