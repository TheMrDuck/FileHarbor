//! organizer-core: filesystem analysis, exact duplicate detection,
//! empty-file detection and a quarantine/restore/delete pipeline.
//!
//! Safety model: this crate never permanently deletes an original user
//! file. Permanent deletion operates exclusively on files that have
//! already been moved into the application-managed quarantine area.

pub mod cancel;
pub mod deleter;
pub mod dupdetect;
pub mod fs_layer;
pub mod hasher;
pub mod logutil;
pub mod models;
pub mod ops;
pub mod quarantine;
pub mod restore;
pub mod scanner;
pub mod settings;
pub mod store;

pub use cancel::CancelToken;
