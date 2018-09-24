//! Some reexports from `futures` and `tokio` crates.

pub use futures::{Future, Stream};
pub use futures::future::{lazy, poll_fn};
pub use tokio::{run, spawn};
