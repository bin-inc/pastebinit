use std::fmt::Display;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub struct AppError {
    message: String,
    exit_code: i32,
}

impl AppError {
    pub fn input(message: impl Display) -> Self {
        Self {
            message: message.to_string(),
            exit_code: 1,
        }
    }

    pub fn usage(message: impl Display) -> Self {
        Self {
            message: message.to_string(),
            exit_code: 2,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
