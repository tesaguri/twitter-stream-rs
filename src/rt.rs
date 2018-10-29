//! Some reexports from `futures` and `tokio` crates.

extern crate tokio;

pub use self::tokio::{run, spawn};
pub use futures::future::{lazy, poll_fn};
pub use futures::{Future, Stream};
