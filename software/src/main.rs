mod home_manager;
use home_manager::{HomeManager, HomeManagerError};

const INIT_ATTEMPTS:u8 = 5;
const ATTEMPT_DELAY_S:u64 = 60;

fn main() -> Result<(), HomeManagerError>{
    let mut home_manager = HomeManager::new()?;
    home_manager.run()
}