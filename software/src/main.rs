mod home_manager;

use std::{thread::sleep, time::Duration};

use home_manager::{HomeManager, HomeManagerError, IoTError};
use time::{OffsetDateTime, Time, macros::time};

const INIT_ATTEMPTS:u8 = 5;
const ATTEMPT_DELAY_S:u64 = 60;
const TARIFF_START:Time = time!(23:30);
const TARIFF_END:Time = time!(5:30);

#[derive(Clone)]
enum State{
    Tariff,
    Standard
}

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let mut home_manager = match  HomeManager::new() {
        Ok(hm) => hm,
        Err(hm_e) => match hm_e {
            HomeManagerError::IoTError(iot_hm_e) => match iot_hm_e{
                IoTError::Endpoint(iot_e) => {
                        let mut tries:u8 = 1;
                        println!("Error fetching initial data from endpoints due to: {:?} \nRetrying in {ATTEMPT_DELAY_S} second(s)...", iot_e);
                        loop{
                            sleep(Duration::from_secs(ATTEMPT_DELAY_S));
                            tries += 1;
                            let err = match HomeManager::new(){
                                Ok(hm) => break hm,
                                Err(re_err) => re_err,
                            };

                            match err{
                                HomeManagerError::IoTError(iot_e) => match iot_e{
                                    IoTError::Endpoint(re_e) => {
                                        println!("Attempt {tries}/{INIT_ATTEMPTS} failed to fetch initial data from endpoints due to:{:?}", re_e);
                                    },
                                    _ => return Err(Box::new(iot_e))
                                },
                                _ => return Err(Box::new(err))
                            };

                            if tries == INIT_ATTEMPTS {
                                println!("Failed to fetch initial data after {INIT_ATTEMPTS} attempts.");
                                return Err(Box::new(iot_e));
                            }
                            
                            println!("Retrying again in {ATTEMPT_DELAY_S} second(s)...");
                        }
                    },
                _ => {
                    return Err(Box::new(iot_hm_e));
                }
            },
            _ => return Err(Box::new(hm_e))
        }
    };

    home_manager.start_benchmark();

    // TODO: move all this logic into the homemanager class
    
    let mut state:State = State::Standard; // always set to standard, since HomeManager starts on the edge, and standard -> standard only manages EV state

    loop{
        match home_manager.gpio_rx.recv_timeout(Duration::from_secs(10)){
            Ok(pin) => {
                home_manager.tgl_pin(pin);
                println!("Rising edge on pin {pin}");
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let now = OffsetDateTime::now_local()?;
                let new_state = if now.time() > TARIFF_END && now.time() < TARIFF_START {State::Standard} else {State::Tariff};

                match (state.clone(), new_state.clone()) {
                    (State::Tariff, State::Standard) => {
                        home_manager.tariff_end()?;
                    },
                    (State::Tariff, State::Tariff) => {
                        home_manager.tariff_update()?;
                    },
                    (State::Standard, State::Tariff) => {
                        home_manager.tariff_start()?;
                    },
                    (State::Standard, State::Standard) => ()
                }

                home_manager.update_ev_charger()?;

                state = new_state;
            },
            Err(e) => {
                return Err(Box::new(e));
            }
        }
    }
}