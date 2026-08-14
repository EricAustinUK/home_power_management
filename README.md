# Home Power Management

A bespoke home power management system built on a Raspberry Pi Zero W, optimised by edge AI and written in Rust.
Predicts solar generation with an on-device ML model, then calculates and schedules battery and EV charging around a cheap energy tariff window via Home Assistant. 

## How it works

- **Solar prediction (`ml/`)**: a compact regression model trained on historical weather and solar pv generation data predicts the next 24 hours of solar output. Inference runs locally on the Pi Zero W (ARMv6, 512 MB RAM).
- **Weather input(`software/home_manager/weather_data.rs`)**: hourly forecasts from the OpenMeteo API feed the model.
- **Battery scheduling**: during tariff windows, the expected house load is compared against predicted solar, the battery is configured to charge to cover any shortfall, then reset to its baseline when the tariff ends.
- **EV conditional charging**: reads an EV's battery level, home location, and plug state from Home Assistant to control an IoT enabled charger, switching it off once the target charge (currently 95%) is reached.
- **Control panel (`control_panel.rs`)**: a physical panel (currently on a breadboard) with three "appliance" toggle switches (though only one of these is currently configured, being the one representing the home being inhabited) that feed into the estimate for home energy consumption. Shows the state for each these appliances with an LED. 

## Optimization for ARM

The model and inference path were developed with the Pi Zero W's hardware limitations in mind.

| Metric | Value |
|---|---|
| Model size on disk | 282B |
| Model data size on disk | 99KB |
| Inference latency (Pi Zero W) | 1.51ms |
| RAM footprint | 3664KB |

## Hardware

- Raspberry Pi Zero W
- Custom control panel housing 3 push switches with pull-ups, status LEDs
- Home Assistant instance with battery, EV, and charger integrations

## Setup

1. Clone and copy `.env.example` to `.env`, filling in:
   - `HASS_IP`, `HASS_PORT`, `HASS_TOKEN` (long-lived access token obtained in Home Assistant developer settings)
   - `BATTERY_NAME`, `EV_NAME`, `EV_CHARGER_NAME` - entity prefixes as they appear in Home Assistant. Current implementation is for an Ecoflow battery, a BYD EV and a VOLDT charger connected to SmartLife; using [hass-byd-vehicle](https://github.com/jkaberg/hass-byd-vehicle), [hassio-ecoflow-cloud](https://github.com/tolwi/hassio-ecoflow-cloud), HomeAssistant's official Tuya SmartLife integration
   - `WEATHER_API_URL`, `PANEL_LATITUDE`, `PANEL_LONGITUDE` - for the weather forecast and history. Currently only OpenMeteo is supported
   - `MODEL_FILENAME`, `MODEL_DATA_FILENAME` - trained model binary if training off the target is preferred (see `ml/`)
2. Cross-compile for the Pi Zero W:
   ```bash
   cd software
   cargo build --target arm-unknown-linux-musleabihf --release
   ```
   (requires the `arm-linux-gnueabihf-gcc` linker)
3. Deploy and run (uses `PI_USER` and `PI_HOST` from `.env`):
   ```bash
   set -a && source ../.env && set +a
   scp target/arm-unknown-linux-musleabihf/release/home_assist $PI_USER@$PI_HOST:~/
   ssh $PI_USER@$PI_HOST "chmod +x ~/home_assist && ~/home_assist"
   ```
   (also available as the "deploy to pi" / "run on pi" tasks in `.vscode/tasks.json`)

## License

MIT, see [LICENSE](LICENSE)
