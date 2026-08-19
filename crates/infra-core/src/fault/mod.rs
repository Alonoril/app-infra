//! Low-level fixed-size fault primitives.
//!
//! `fault` keeps the compact [`Fault`] representation for stable machine
//! identity and uses [`FaultReport`] as the standard result error so boundary
//! logs can print human-readable messages.

mod code;
mod core;
mod detail;
mod report;

pub use code::{FaultCode, FaultMeta, InfraFault};
pub use core::{Fault, FaultParts};
pub use detail::{FaultDetail, InfraDetail, RawFaultDetail};
pub use report::FaultReport;

/// Standard result type for infrastructure paths with message-aware errors.
pub type FaultResult<T> = Result<T, FaultReport>;
