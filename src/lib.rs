pub mod cli;
pub mod config;
pub mod error;
pub mod input;
pub mod platform;
pub mod posting;
pub mod preferences;
pub mod transport;

pub use error::{AppError, AppResult};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
