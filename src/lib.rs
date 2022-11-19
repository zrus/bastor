//! # Bastor
//! An trait implementation that helps implementing actors
//! with self-implemented state when using Bastion.
//!

// Force missing implementations
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
// Deny using unsafe code
#![deny(unsafe_code)]
// Doc generation experimental features
#![cfg_attr(feature = "docs", feature(doc_cfg))]

mod actor;
mod error;

pub use crate::actor::Actor;
pub use crate::error::Error;
