use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to generate plot: {0}")]
    PlotGenerationFailed(String),
    #[error("Global settings are missing in context.")]
    MissingGlobalSettings,
    #[error("No quotes available")]
    NoQuotesAvailable,
    #[error("Database request failed")]
    DatabaseRequestFailed(String),
}
