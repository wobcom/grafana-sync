use tracing::instrument;
use crate::config::Config;

struct SyncService {
    config: Config
}

impl SyncService {
    #[instrument]
    pub fn new(config: Config) -> Self {
        Self {
            config
        }
    }

    #[instrument]
    pub fn run() {

    }
}