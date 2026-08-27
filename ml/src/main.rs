mod rill_structs;
use std::path::Path;

use linfa_elasticnet::ElasticNet;
use rill_structs::*;
use ndarray::{Axis, Array1, Array2, s};
use linfa::prelude::*;

use csv::ReaderBuilder;

fn load_x(path: &str) -> Array2<f64> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .unwrap();

    let rows: Vec<Vec<f64>> = rdr
        .records()
        .map(|r| {
            let record = r.unwrap();

            // Skip time column, parse remaining 9 columns
            record
                .iter()
                .skip(1)
                .map(|v| v.parse::<f64>().unwrap())
                .collect()
        })
        .collect();

    let nrows = rows.len();
    let ncols = rows[0].len();

    let flat: Vec<f64> = rows.into_iter().flatten().collect();

    Array2::from_shape_vec((nrows, ncols), flat).unwrap()
}

fn load_y(path: &str) -> Array1<f64> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .unwrap();

    let values: Vec<f64> = rdr
        .records()
        .map(|r| {
            let record = r.unwrap();
            record[1].parse::<f64>().unwrap()
        })
        .collect();

    Array1::from_vec(values)
}

fn main() {
    println!("{}", std::env::current_dir().unwrap().display());
    let x = load_x("data/weather_features.csv");
    let y = load_y("data/power_generation.csv");

    let mean = x.mean_axis(Axis(0)).unwrap();
    let var = x.var_axis(Axis(0), 0.0);
    let s_dev = var.mapv(|v:f64| v.sqrt());

    let mut x_scaled = x.clone();
    for mut row in x_scaled.rows_mut() {
        row -= &mean;
        row /= &s_dev;
    }

    let dataset = Dataset::new(x_scaled, y.clone());

    let reg_model = ElasticNet::params()
        .l1_ratio(0.)
        .penalty(500.)
        .fit(&dataset)
        .unwrap();

    let weights = reg_model.hyperplane().to_vec();
    let intercept = reg_model.intercept();

    let feature_count = 9;
    let samples_seen = x.nrows() as u64;

    let m2s: Vec<f64> = var.iter()
        .map(|&v| v * (samples_seen as f64 - 1.0))
        .collect();

    let stored_model = MockRegressionPipeline {
        transformer: MockStandardScaler {
            feature_count,
            config: MockStandardScalerConfig {
                with_mean: true,
                with_std: true,
                epsilon: 1e-12,
            },
            counts: vec![samples_seen; feature_count],
            means: mean.to_vec(),
            m2s,
        },
        model: MockLinearRegression {
            feature_count,
            weights: weights,
            intercept: intercept,
            optimizer: MockOptimizer::Sgd(MockSgd {
                feature_count,
                config: MockSgdConfig {
                    learning_rate: 0.001,
                    l2: 0.001,
                },
                samples_seen,
            }),
            loss: MockRegressionLoss::SquaredError,
            samples_seen,
        },
    };

    let stored_snapshot = SnapshotMirror {
        format_version: 1,
        model: stored_model
    };

    let model_path = Path::new("output/model.bin");

    std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    std::fs::write(model_path, postcard::to_allocvec(&stored_snapshot).unwrap()).unwrap();
    println!("Model saved successfully");

    let samples = x.nrows();

    let mut training_data = Array2::<f64>::zeros((samples, 10));

    training_data
        .slice_mut(s![.., 0..9])
        .assign(&x);

    training_data
        .column_mut(9)
        .assign(&y);

    let training_data_vec: Vec<[f64; 10]> = training_data
        .outer_iter()
        .map(|row| row.to_vec().try_into().unwrap())
        .collect();
    
    let bytes = postcard::to_allocvec(&training_data_vec).unwrap();

    std::fs::write(
        "output/model_data.bin",
        bytes
    ).unwrap();


}