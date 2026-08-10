mod rill_structs;
use rill_structs::*;
use ndarray::{Axis, Array1, Array2};
use linfa::prelude::*;

fn main() {
    let x = Array2::from_shape_vec((2, 7), vec![
        0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6,
        0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7
    ]).unwrap();

    let mean = x.mean_axis(Axis(0)).unwrap();
    let var = x.var_axis(Axis(0), 0.0);
    let s_dev = var.mapv(|v:f64| v.sqrt());

    let mut x_scaled = x.clone();
    for mut row in x_scaled.rows_mut() {
        row -= &mean;
        row /= &s_dev;
    }

    let y = Array1::from_vec(vec![67.0, 69.0]);
    let dataset = Dataset::new(x_scaled, y);
    let lin_reg = linfa_linear::LinearRegression::new().fit(&dataset).unwrap();

    let feature_count = 7;
    let samples_seen = 2;
    
    let m2s: Vec<f64> = var.iter().map(|&v| v * samples_seen as f64).collect();

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
            weights: lin_reg.params().to_vec(),
            intercept: lin_reg.intercept(),
            optimizer: MockOptimizer::Sgd(MockSgd {
                feature_count,
                config: MockSgdConfig {
                    learning_rate: 0.1,
                    l2: 0.0,
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

    std::fs::write("output/model.bin", postcard::to_allocvec(&stored_snapshot).unwrap()).unwrap();
    println!("Model saved successfully");
}