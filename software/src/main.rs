mod home_manager;

use std::{thread::sleep, time::Duration};

use home_manager::{HomeManager, InitError};

pub const INIT_ATTEMPTS:u8 = 5;
pub const ATTEMPT_DELAY_S:u64 = 60;

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let home_manager = match  HomeManager::new() {
        Ok(hm) => hm,
        Err(e) => match e {
            InitError::Endpoint(e) => {
                let mut tries:u8 = 1;
                println!("Error fetching initial data from endpoints due to: {:?} \nRetrying in {ATTEMPT_DELAY_S} second(s)...", e);
                loop{
                    sleep(Duration::from_secs(ATTEMPT_DELAY_S));
                    tries += 1;
                    let err:InitError = match HomeManager::new(){
                        Ok(hm) => break hm,
                        Err(re_err) => re_err,
                    };

                    match err{
                        InitError::Endpoint(re_e) => {
                            println!("Attempt {tries}/{INIT_ATTEMPTS} failed to fetch initial data from endpoints due to:{:?}", re_e);
                        },
                        _ => return Err(Box::new(err))
                    };

                    if tries == INIT_ATTEMPTS {
                        println!("Failed to fetch initial data after {INIT_ATTEMPTS} attempts.");
                        return Err(Box::new(e));
                    }
                    
                    println!("Retrying again in {ATTEMPT_DELAY_S} second(s)...");
                }
            },
            _ => {
                return Err(Box::new(e));
            }
        }
    };

    std::thread::park();

    Ok(())
}