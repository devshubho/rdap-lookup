//! Error type shared across the crate.

use thiserror::Error;

/// Errors that can arise while fetching, parsing, or querying RDAP data.
#[derive(Debug, Error)]
pub enum Error {
    /// A network or HTTP-level failure.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// A filesystem failure while reading or writing the cache or overlay.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON parse or serialize failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A URL could not be parsed.
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),

    /// The input was not a usable domain name.
    #[error("invalid domain: {0}")]
    InvalidDomain(String),

    /// No RDAP server is known for the TLD.
    #[error("no RDAP server known for .{0}")]
    NoServer(String),
}

/// Convenience alias for `Result` with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
