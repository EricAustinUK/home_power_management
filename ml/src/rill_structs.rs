use serde::Serialize;

#[derive(Serialize)]
pub struct SnapshotMirror<T> {
    pub format_version: u32,
    pub model: T,
}

#[derive(Serialize)]
pub struct MockStandardScalerConfig {
    pub with_mean: bool,
    pub with_std: bool,
    pub epsilon: f64,
}

#[derive(Serialize)]
pub struct MockStandardScaler {
    pub feature_count: usize,
    pub config: MockStandardScalerConfig,
    pub counts: Vec<u64>,
    pub means: Vec<f64>,
    pub m2s: Vec<f64>,
}

#[derive(Serialize)]
pub enum MockRegressionLoss {
    SquaredError,
}

#[derive(Serialize)]
pub struct MockSgdConfig {
    pub learning_rate: f64,
    pub l2: f64,
}

#[derive(Serialize)]
pub struct MockSgd {
    pub feature_count: usize,
    pub config: MockSgdConfig,
    pub samples_seen: u64,
}

#[derive(Serialize)]
pub enum MockOptimizer {
    Sgd(MockSgd),
}

#[derive(Serialize)]
pub struct MockLinearRegression {
    pub feature_count: usize,
    pub weights: Vec<f64>,
    pub intercept: f64,
    pub optimizer: MockOptimizer,
    pub loss: MockRegressionLoss,
    pub samples_seen: u64,
}

#[derive(Serialize)]
pub struct MockRegressionPipeline {
    pub transformer: MockStandardScaler,
    pub model: MockLinearRegression,
}