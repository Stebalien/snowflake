#![cfg_attr(test, feature(test, std_misc))]
//! A crate for quickly generating unique IDs with guaranteed properties.
//!
//! This crate currently includes guaranteed process unique IDs but may include new ID types in the
//! future.
mod process_unique_id;

pub use process_unique_id::ProcessUniqueId;
