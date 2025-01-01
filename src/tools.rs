use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct SuitabilityError(pub &'static str);
