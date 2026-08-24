use std::path::PathBuf;

use crate::home_manager::{iot_controller::{WeatherData, WeatherDataError}};
use rill_ml::{
    OnlineRegressor, RillError, Transformer, models::{LinearRegression,
    LinearRegressionConfig}, optim::{Optimizer, SgdConfig}, persistence::Snapshot, pipeline::RegressionPipeline, preprocessing::StandardScaler
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
    FileReadError(#[from] std::io::Error),

    #[error("Path did not convert to string")]
    PathToStringError()
}

pub struct WeatherFeatures{
    pub features:[[f64;9];24]
}

type Model = RegressionPipeline<StandardScaler, LinearRegression>;

pub struct MLEngine{
    model:Model,
    data_path:PathBuf,
    data_bak_path:PathBuf
}

impl MLEngine{
    pub fn new(model_bytes:Option<Vec<u8>>, model_data_path_str:Option<String>) -> Result<Self, MLError>{
        match model_bytes{
            Some(bytes) => {
                let snapshot: Snapshot<Model> = postcard::from_bytes(&bytes)?;
                let valid_model = snapshot.into_validated_model()?;
                
                println!("Valid model loaded.");

                let model_data_path = match model_data_path_str{
                    Some(str) => {
                        println!("Found model training data in filesystem.");
                        PathBuf::from(&str)
                    },
                    None => {
                        std::fs::write("model_data.bin", vec![])?;
                        PathBuf::from("model_data.bin")
                    }
                };

                let model_data_bak_path = match model_data_path.to_str(){
                    Some(str) => PathBuf::from(&format!("{str}.bak")),
                    None => return Err(MLError::PathToStringError())
                };

                Ok(MLEngine { model: valid_model, data_path:model_data_path, data_bak_path:model_data_bak_path })
            },
            _ => {
                println!("No model passed in .env file. Starting a fresh model:");
                let scaler = StandardScaler::new(9)?;
                let mut sgd_conf = SgdConfig::default();
                sgd_conf.learning_rate = 0.001;
                sgd_conf.l2 = 0.01;
                let optimiser = Optimizer::sgd(9,sgd_conf)?;
                let mut lr_conf = LinearRegressionConfig::default();
                lr_conf.optimizer = optimiser;
                let regression = LinearRegression::new(9, lr_conf)?;        
                let mut model:Model = RegressionPipeline::new(scaler, regression)?;

                let model_data_path = match model_data_path_str{
                    Some(str) => {
                        let features_set:Vec<[f64;10]> = postcard::from_bytes(&std::fs::read(&str)?)?;
                        println!("No model found, but valid historic data found. Training  model.");
                        let train_scaler = StandardScaler::new(9)?;
                        for raw_features in features_set {
                            let features = train_scaler.transform(&raw_features[0..9])?;
                            model.learn(&features[0..9], raw_features[9])?;
                        }
                        println!("Model training complete.");
                        PathBuf::from(&str)
                    },
                    None => {
                        std::fs::write("model_data.bin", vec![])?;
                        PathBuf::from("model_data.bin")
                    }
                };

                let model_data_bak_path = match model_data_path.to_str(){
                    Some(str) => PathBuf::from(&format!("{str}.bak")),
                    None => return Err(MLError::PathToStringError())
                };
                
                Ok(MLEngine { model: model, data_path:model_data_path, data_bak_path:model_data_bak_path })
            }
        }
    }

    pub fn infer(&self, weather:&WeatherData) -> Result<[f64; 24], MLError> {
        let mut result_arr:[f64; 24] = [0.; 24];
        let features:WeatherFeatures = weather.clone().try_into()?;
        for i in 0..24{
            let result = self.model.predict(features.features.get(i).ok_or(WeatherDataError::DataOverflow())?)?;
            *result_arr.get_mut(i).ok_or(WeatherDataError::DataOverflow())? = result;
        }
        Ok(result_arr)
    }

    pub fn train(&mut self, real_weather:&WeatherData, real_solar:&[f64; 24]) -> Result<(), MLError>{
        let features:WeatherFeatures = real_weather.clone().try_into()?;

        // TODO: Include backup file
        match std::fs::read(&self.data_path){
            Ok(buf) => {
                let mut current_data:Vec<[f64; 10]> = postcard::from_bytes(&buf)?;
                let mut new_data:Vec<[f64; 10]> = features.features.iter().zip(real_solar).map(|(f, o)| {
                    let mut ret = [0.; 10];
                    for i in 0..9{
                        ret[i] = f[i];
                    }
                    ret[9] = *o;
                    ret
                })
                .collect();
                current_data.append(&mut new_data);
                std::fs::write(&self.data_path, postcard::to_allocvec(&current_data)? )?;
            },
            Err(_) => println!("Error reading model file. Continuing to training step")
        };

        for i in 0..24{
            self.model.learn(&features.features[i], real_solar[i])?;
        }
        Ok(())
    }
}