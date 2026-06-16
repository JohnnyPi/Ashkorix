use ashkorix_core::app::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppStateWrapper(pub Arc<Mutex<AppState>>);

impl AppStateWrapper {
    pub fn new() -> Result<Self, ashkorix_core::error::AshkorixError> {
        Ok(Self(Arc::new(Mutex::new(AppState::new()?))))
    }
}
