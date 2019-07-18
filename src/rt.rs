//! Some reexports from `futures` and `tokio` crates.

pub use futures::future::{lazy, poll_fn};
pub use futures::{Future, Stream};
pub use tokio::{run, spawn};
