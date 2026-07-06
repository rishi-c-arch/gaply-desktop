use std::path::PathBuf;

use crate::error::GaplyError;

/// Runtime configuration for the core. The shell (Tauri, CLI, …) decides
/// the values; the core only validates and consumes them.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Where the SQLite database file lives.
    pub db_path: PathBuf,
    /// Default tracing filter when RUST_LOG is not set.
    pub log_filter: String,
}

impl AppConfig {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
            log_filter: "info,app_lib=debug,gaply_core=debug".to_string(),
        }
    }

    pub fn validate(&self) -> Result<(), GaplyError> {
        if self.db_path.as_os_str().is_empty() {
            return Err(GaplyError::Config("db_path must not be empty".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_db_path_is_rejected() {
        let cfg = AppConfig::new("");
        assert!(matches!(cfg.validate(), Err(GaplyError::Config(_))));
    }

    #[test]
    fn valid_config_passes() {
        assert!(AppConfig::new("/tmp/gaply.db").validate().is_ok());
    }
}
