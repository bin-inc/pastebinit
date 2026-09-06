pub mod error;

pub use error::{AppError, AppResult};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
