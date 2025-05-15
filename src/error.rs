pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    JSON(#[from] serde_json::error::Error),
    #[error("The config was invalid. Key \"{0}\" not found.")]
    ConfigKeyMissing(String),
    #[error("The config was invalid. Key \"{0}\" was not of type \"{1}\".")]
    ConfigKeyTypeWrong(String, &'static str),
    #[error(transparent)]
    ConfigError(#[from] crate::config::ConfigError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(
        "The provided static header value was invalid. This is most likely a configuration error."
    )]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error("The supplied synchronization duration interval is out of range. (0 < interval < 9223372036854775808)")]
    SyncIntervalOutOfRange,
    #[error(transparent)]
    ChronoOutOfRange(#[from] chrono::OutOfRangeError),
    #[error("The local uid was not defined in the peer instance list")]
    LocalNotInPeerList,
    #[error(transparent)]
    Federation(#[from] grafana_sync_federation::Error),
}
