use crate::home_manager::{iot_controller::{WeatherData, WeatherDataError}};
use rill_ml::{
    OnlineRegressor,
    models::{LinearRegression,
    LinearRegressionConfig},
    optim::{Optimizer, SgdConfig},
    pipeline::RegressionPipeline,
    preprocessing::StandardScaler,
    persistence::{Snapshot},
    RillError
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MLError {
    #[error("Model error: {0}")]
    ModelError(#[from] RillError),

    #[error("Invalid data error")]
    DataError(#[from] WeatherDataError),
    
    #[error("Failed to parse model from file")]
    ModelParseError(#[from] postcard::Error),


    #[error("Failed to read model file")]
    FileReadError(#[from] std::io::Error)
}

pub struct WeatherFeatures{
    pub features:[[f64;9];24]
}

type Model = RegressionPipeline<StandardScaler, LinearRegression>;

pub struct MLEngine{
    model:Model
}

impl MLEngine{
    pub fn new(model_bytes:Option<Vec<u8>>) -> Result<Self, MLError>{
        match model_bytes{
            Some(bytes) => {
                let snapshot: Snapshot<Model> = postcard::from_bytes(&bytes)?;

                let valid_model = snapshot.into_validated_model()?;

                Ok(MLEngine { model: valid_model })
            },
            _ => {
                println!("No model passed in .env file. Starting a fresh model:");
                let scaler = StandardScaler::new(9)?;
                let mut sgd_conf = SgdConfig::default();
                sgd_conf.learning_rate = 0.1;
                let optimiser = Optimizer::sgd(9,sgd_conf)?;
                let mut lr_conf = LinearRegressionConfig::default();
                lr_conf.optimizer = optimiser;
                let regression = LinearRegression::new(9, lr_conf)?;        
                let model:Model = RegressionPipeline::new(scaler, regression)?;
                
                Ok(MLEngine { model:model })
            }
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