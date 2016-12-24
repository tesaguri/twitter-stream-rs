use futures::{Future, Sink, Stream};
use futures::stream::{self, AndThen, MapErr};
use futures::sync::mpsc::{self, Receiver, Sender};
use std::io::BufRead;
use std::thread;
use Error;

type Result = ::Result<String>;

/// A stream over each non-empty line on a `BufRead`.
pub type Lines = AndThen<
    MapErr<
        Receiver<Result>,
        fn(()) -> Error
    >,
    fn(Result) -> Result,
    Result
>;

/// Adds to `Sender` an ability to send an `Error` to its corresponding `Receiver` while panicking.
struct SenderPanicGuard(Option<Sender<Result>>);

/// Returns a stream over each non-empty line on `a`.
#[allow(unused_variables)]
pub fn lines<A: BufRead + Send + 'static>(a: A) -> Lines {
    let (tx, rx) = mpsc::channel(8);

    thread::Builder::new().name("twitter_user_stream_sender".into()).spawn(move || {
        let txg = SenderPanicGuard(Some(tx.clone()));
        let stream = stream::iter(a.lines().map(|r| Ok(r.map_err(Error::from))));
        tx.send_all(stream).wait().unwrap();
    }).unwrap();

    let rx = rx.map_err(err_map as fn(_) -> _).and_then(id as fn(_) -> _);
    fn err_map(_: ()) -> Error { panic!("Receiver failed") }
    fn id(r: Result) -> Result { r }

    rx
}

impl Drop for SenderPanicGuard {
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        if thread::panicking() {
            if let Some(tx) = self.0.take() {
                tx.send(Err(Error::InternalPanicError)).wait();
            }
        }
    }
}
