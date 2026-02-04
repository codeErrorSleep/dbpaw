use crate::db::local::LocalDb;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub local_db: Arc<Mutex<Option<LocalDb>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            local_db: Arc::new(Mutex::new(None)),
        }
    }
}
