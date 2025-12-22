use thiserror::Error;

#[derive(Error, Debug)]
pub enum WinselError {
    #[error("kitty command failed: {0}")]
    KittyCommandFailed(String),

    #[error("no kitty socket available")]
    NoKittySocket,

    #[error("window not found: {0}")]
    WindowNotFound(String),

    #[error("tab not found: {0}")]
    TabNotFound(String),

    #[error("fzf not installed")]
    FzfNotFound,

    #[error("clipboard operation failed")]
    ClipboardFailed,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
}

pub type Result<T> = std::result::Result<T, WinselError>;
