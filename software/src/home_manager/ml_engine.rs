use std::path::Path;

use crate::home_manager::iot_controller::{WeatherData, WeatherDataError};
use rill_ml::{
    OnlineRegressor,
    models::{LinearRegression,
    LinearRegressionConfig},
    optim::{Optimizer, SgdConfig},
    pipeline::RegressionPipeline,
    preprocessing::StandardScaler,
    persistence::Snapshot
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MLError {
    #[error("Model error: {0}")]
    ModelError(#[from] rill_ml::RillError),

    #[error("Invalid data error")]
    DataError(#[from] WeatherDataError),
    
    #[error("Failed to parse model from file")]
    ModelParseError(#[from] postcard::Error),


    #[error("Failed to read model file")]
    FileReadError(#[from] std::io::Error)
}

pub struct WeatherFeatures{
    pub features:[[f64;7];24]
}

type Model = RegressionPipeline<StandardScaler, LinearRegression>;

pub struct MLEngine{
    model:Model
}

impl MLEngine{
    pub fn new(model_bytes:Option<Vec<u8>>) -> Result<Self, MLError>{
        let scaler = StandardScaler::new(7)?;
        
        let mut sgd_conf = SgdConfig::default();
        sgd_conf.learning_rate = 0.05;
        sgd_conf.l2 = 0.00;

        let optimiser = Optimizer::sgd(7,sgd_conf)?;

        let mut lr_conf = LinearRegressionConfig::default();
        lr_conf.optimizer = optimiser;

        let regression = LinearRegression::new(7, lr_conf)?;
        match model_bytes{
            Some(model_bytes) => {
                let snapshot:Snapshot<Model> =  postcard::from_bytes(&model_bytes)?; 
                Ok(MLEngine { model:snapshot.model })
            },
            _ => Ok(MLEngine { model:RegressionPipeline::new(scaler, regression)? })
        }
    }

    pub fn infer(&self, weather:WeatherData) -> Result<[f64; 24], MLError> {
        let mut result_arr:[f64; 24] = [0.; 24];
        let features:WeatherFeatures = weather.try_into()?;
        for i in 0..24{
            let result = self.model.predict(features.features.get(i).ok_or(WeatherDataError::DataOverflow())?)?;
            *result_arr.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = result;
        }
        Ok(result_arr)
    }

    pub fn train(&mut self, real_weather:WeatherData, real_solar:[f64; 24]) -> Result<(), MLError>{
        let features:WeatherFeatures = real_weather.try_into()?;
        for i in 0..24{
            self.model.learn(&features.features[i], real_solar[i])?;
        }
        Ok(())
    }
}