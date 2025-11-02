use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to generate plot: {0}")]
    PlotGenerationFailed(String),

    #[error("Global settings are missing in context.")]
    MissingGlobalSettings,

    #[error("Failed access database")]
    DatabaseAccessFailed,

    #[error("No quotes available")]
    NoQuotesAvailable,
}
