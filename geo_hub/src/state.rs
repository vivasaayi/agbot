use crate::{config::HubConfig, db::DbPool};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: Arc<HubConfig>,
}
