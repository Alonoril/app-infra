//! Shared application result and error-code primitives.

mod code;
mod error;

pub use code::{AppErrCode, ErrCodeTrait, SysErr};
pub use error::AppError;

/// Standard result type for infrastructure code.
pub type AppResult<T> = Result<T, AppError>;
