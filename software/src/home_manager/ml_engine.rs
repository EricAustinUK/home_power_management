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

    pub fn infer(&self, weather:WeatherData) -> Result<Vec<f64>, MLError> {
        let mut result_vec = Vec::new();
        let feature_arrs = weather.hourly;
        for i in 0..feature_arrs.hour_sin.len(){
            let result = self.model.predict(&[
                (*feature_arrs.hour_sin.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                (*feature_arrs.hour_cos.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                (*feature_arrs.shortwave_radiation.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                (*feature_arrs.direct_radiation.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                (*feature_arrs.diffuse_radiation.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                (*feature_arrs.cloud_cover.get(i).ok_or(WeatherDataError::FloatConversion())? as f64),
                (*feature_arrs.temperature_2m.get(i).ok_or(WeatherDataError::FloatConversion())? as f64)
            ])?;
            result_vec.push(result);
        }
        Ok(result_vec)
    }
}