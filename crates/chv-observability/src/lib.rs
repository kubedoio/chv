use tracing::Span;

pub fn init_logger(filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

pub fn operation_span(op_id: &str) -> Span {
    tracing::info_span!("operation", operation_id = op_id)
}

#[derive(Debug, Clone, Default)]
pub struct Metrics;

impl Metrics {
    pub fn new() -> Self {
        Self
    }

    pub fn increment_counter(&self, name: &'static str) {
        metrics::counter!(name).increment(1);
    }

    pub fn gauge(&self, name: &'static str, value: f64) {
        metrics::gauge!(name).set(value);
    }
}
