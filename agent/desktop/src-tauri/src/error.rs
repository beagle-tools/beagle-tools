//! One error type across the IPC boundary.
//!
//! Every message here is written for the person reading it in the UI's
//! error bar, not for a log grep: it says what failed and what they could
//! do about it. Tauri needs `Serialize` to return an `Err` to the
//! frontend, so this is a newtype over the message rather than an enum —
//! the frontend never branches on the kind, it displays it.

use std::fmt;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppError {
    pub message: String,
}

impl AppError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

/// A poisoned lock means another command panicked while holding it. That is
/// a bug, but it must not take the whole app down — report it and let the
/// user retry.
pub fn lock_failed(what: &str) -> AppError {
    AppError::new(format!(
        "internal lock on the {what} was poisoned by an earlier failure — restart Agentdesk"
    ))
}
