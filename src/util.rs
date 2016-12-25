use futures::{Future, Poll, Sink, Stream};
use futures::stream::{self, AndThen, MapErr};
use futures::sync::mpsc::{self, Receiver, Sender};
use std::io::BufRead;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
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

/// A future which resolves at a specific period of time.
pub struct Timeout {
    when: Instant,
    parked: bool,
    is_active: Arc<AtomicBool>,
}

/// Adds to `Sender` an ability to send an `Error` to its corresponding `Receiver` while panicking.
struct SenderPanicGuard(Option<Sender<Result>>);

/// Returns a stream over each non-empty line on `a`.
#[allow(unused_variables)]
pub fn lines<A: BufRead + Send + 'static>(a: A) -> Lines {
    let (tx, rx) = mpsc::channel(8);

    thread::Builder::new().name("twitter_user_stream_sender".into()).spawn(move || {
        let txg = SenderPanicGuard(Some(tx.clone()));
        let iter = a.lines().map(|r| Ok(r.map_err(Error::from)));
        let stream = stream::iter(iter);
        tx.send_all(stream).wait().unwrap();
    }).unwrap();

    let rx = rx.map_err(err_map as fn(_) -> _).and_then(id as fn(_) -> _);
    fn err_map(_: ()) -> Error { panic!("Receiver failed") }
    fn id(r: Result) -> Result { r }

    rx
}

impl Timeout {
    pub fn after(dur: Duration) -> Self {
        Timeout::at(Instant::now() + dur)
    }

    pub fn at(at: Instant) -> Self {
        Timeout {
            when: at,
            parked: false,
            is_active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn park(&mut self, now: Instant) {
        use futures::task;

        if !self.parked {
            let wait = self.when - now; // Panics if `self.when < now`.
            let is_active = self.is_active.clone();
            let task = task::park();
            thread::spawn(move || {
                thread::sleep(wait);
                if is_active.load(Ordering::Relaxed) {
                    task.unpark();
                }
            });
            self.parked = true;
        }
    }
}

impl Future for Timeout {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        use futures::Async::*;

        trace!("Timeout::poll");

        let now = Instant::now();
        if now < self.when {
            self.park(now);
            Ok(NotReady)
        } else {
            Ok(Ready(()))
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
    }
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
