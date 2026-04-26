//! Chat orchestration: agent registry, streaming pipeline, cancellation.

pub mod cancel;
pub mod pipeline;

pub use cancel::CancellationRegistry;
