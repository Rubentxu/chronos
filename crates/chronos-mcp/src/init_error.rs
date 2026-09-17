use std::path::PathBuf;

use chronos_services::error::ServiceError;

use crate::server::StoreOpenError;

#[derive(Debug, thiserror::Error)]
pub enum ChronosServerInitError {
    #[error("could not open session store: {0}")]
    StoreOpen(#[from] StoreOpenError),
    #[error("could not bootstrap execution logs at {root}: {cause}")]
    ExecutionLogBootstrap { root: PathBuf, cause: ServiceError },
}
