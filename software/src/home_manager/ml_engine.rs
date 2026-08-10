use crate::home_manager::iot_controller::WeatherData;
use rill_ml::{
    OnlineRegressor, models::{LinearRegression, LinearRegressionConfig}, optim::{Optimizer, SgdConfig}, pipeline::RegressionPipeline, preprocessing::StandardScaler,
};

pub struct MLEngine{
    model:RegressionPipeline<StandardScaler, LinearRegression>
}

impl MLEngine{
    pub fn new() -> Result<Self, rill_ml::RillError>{
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

    pub fn infer(&self, weather:WeatherData) -> usize {
        return 67;
    }
}