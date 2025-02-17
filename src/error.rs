use reqwest::header::InvalidHeaderValue;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GSError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    ParseYaml(#[from] serde_yaml::Error),
    #[error("The config was invalid. Key \"{0}\" not found.")]
    ConfigKeyMissing(String),
    #[error("The config was invalid. Key \"{0}\" was not of type \"{1}\".")]
    ConfigKeyTypeWrong(String, &'static str),
    #[error(transparent)]
    RequestError(#[from] reqwest::Error),
    #[error(
        "The provided static header value was invalid. This is most likely a configuration error."
    )]
    InvalidHeader(#[from] InvalidHeaderValue),
    #[error(transparent)]
    JSONError(#[from] serde_json::error::Error),
    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),
}
