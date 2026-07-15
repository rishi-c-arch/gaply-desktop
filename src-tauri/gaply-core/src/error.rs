use serde::ser::SerializeStruct;

/// Unified error type for all core operations.
///
/// Serializes to `{ code, message }` so the webview gets a stable,
/// machine-matchable shape regardless of which shell wraps the core.
#[derive(Debug, thiserror::Error)]
pub enum GaplyError {
    #[error("{0}")]
    Validation(String),

    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("{0}")]
    Conflict(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("keychain error: {0}")]
    Keychain(String),

    #[error("internal error: {0}")]
    Internal(String),

    /// The user cancelled a long-running operation. A distinct code so the
    /// frontend can treat it as a benign stop, not an error toast.
    #[error("cancelled")]
    Cancelled,
}

impl GaplyError {
    /// Stable machine-readable code, safe to match on from the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            GaplyError::Validation(_) => "validation",
            GaplyError::NotFound { .. } => "not_found",
            GaplyError::Conflict(_) => "conflict",
            GaplyError::Database(_) => "database",
            GaplyError::Config(_) => "config",
            GaplyError::Io(_) => "io",
            GaplyError::Keychain(_) => "keychain",
            GaplyError::Internal(_) => "internal",
            GaplyError::Cancelled => "cancelled",
        }
    }
}

impl serde::Serialize for GaplyError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("GaplyError", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for GaplyError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::SqliteFailure(f, msg)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                GaplyError::Conflict(msg.unwrap_or_else(|| "constraint violation".into()))
            }
            other => GaplyError::Database(other.to_string()),
        }
    }
}

impl From<r2d2::Error> for GaplyError {
    fn from(e: r2d2::Error) -> Self {
        GaplyError::Database(format!("connection pool: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_code_and_message() {
        let err = GaplyError::NotFound { entity: "project", id: "42".into() };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["message"], "project not found: 42");
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(GaplyError::Validation("x".into()).code(), "validation");
        assert_eq!(GaplyError::Conflict("x".into()).code(), "conflict");
        assert_eq!(GaplyError::Database("x".into()).code(), "database");
    }
}
