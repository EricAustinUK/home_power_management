use crate::home_manager::iot_controller::{WeatherData, WeatherDataError};
use rill_ml::{
    OnlineRegressor, models::{LinearRegression, LinearRegressionConfig}, optim::{Optimizer, SgdConfig}, pipeline::RegressionPipeline, preprocessing::StandardScaler,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MLError {
    #[error("Model error: {0}")]
    ModelError(#[from] rill_ml::RillError),

    #[error("Invalid data error")]
    DataError(#[from] WeatherDataError),
    
}

struct Features{
    features:[[f64;7];24]
}

impl TryFrom<WeatherData> for Features{
    type Error = WeatherDataError;
    fn try_from(weather: WeatherData) -> Result<Self, Self::Error> {
        let feature_arrs = weather.hourly;

        let mut hour_sin:[f32; 24] = [0.; 24];
        let mut hour_cos:[f32; 24] = [0.; 24];

        for i in 0..24 {
            let time = &feature_arrs.time[i];
            let hour_str = time.get(11..13).ok_or(WeatherDataError::DateConversion())?;
            let hour:f32 = match hour_str.parse(){
                Ok(h) => h,
                Err(_) => return Err(WeatherDataError::DateConversion())
            };
            let angle:f32 = 2.0 * std::f32::consts::PI * hour / 24.0;

            *hour_sin.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = angle.sin();
            *hour_cos.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = angle.cos();
        }
        let mut result = Self{ features:[[0.; 7];24] };
        for i in 0..24{
            *result.features.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = [
                    (*hour_sin.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                    (*hour_cos.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                    (*feature_arrs.shortwave_radiation.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                    (*feature_arrs.direct_radiation.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                    (*feature_arrs.diffuse_radiation.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                    (*feature_arrs.cloud_cover.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                    (*feature_arrs.temperature_2m.get(i).ok_or(WeatherDataError::FloatConversion())? as f64)
            ];
        }
        Ok(result)
    }
}

pub struct MLEngine{
    model:RegressionPipeline<StandardScaler, LinearRegression>
}

impl MLEngine{
    pub fn new() -> Result<Self, MLError>{
        let scaler = StandardScaler::new(7)?;
        
        let mut sgd_conf = SgdConfig::default();
        sgd_conf.learning_rate = 0.05;
        sgd_conf.l2 = 0.00;

        let optimiser = Optimizer::sgd(7,sgd_conf)?;

        let mut lr_conf = LinearRegressionConfig::default();
        lr_conf.optimizer = optimiser;

        let regression = LinearRegression::new(7, lr_conf)?;

        return Ok(MLEngine { model:RegressionPipeline::new(scaler, regression)? });
    }

    pub fn infer(&self, weather:WeatherData) -> Result<[f64; 24], MLError> {
        let mut result_arr:[f64; 24] = [0.; 24];
        let features:Features = weather.try_into()?;
        for i in 0..24{
            let result = self.model.predict(features.features.get(i).ok_or(WeatherDataError::DataOverflow())?)?;
            *result_arr.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = result;
        }
        Ok(result_arr)
    }

    pub fn train(&mut self, real_weather:WeatherData, real_solar:[f64; 24]) -> Result<(), MLError>{
        let features:Features = real_weather.try_into()?;
        for i in 0..24{
            self.model.learn(&features.features[i], real_solar[i])?;
        }
        Ok(())
    }
}