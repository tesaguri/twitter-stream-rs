use chrono::{TimeZone, UTC};
use futures::{Future, Sink, Stream};
use futures::stream::{self, Then};
use futures::sync::mpsc::{self, Receiver};
use serde::de::{Deserialize, Deserializer, Error};
use std::fmt::{self, Display, Formatter};
use std::thread;
use types::DateTime;

pub type IterStream<T, E> = Then<
    Receiver<Result<T, E>>,
    fn(Result<Result<T, E>, ()>) -> Result<T, E>,
    Result<T, E>,
>;

/// Represents an infallible operation in `default_client::new`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Never {}

macro_rules! string_enums {
    (
        $(
            $(#[$attr:meta])*
            pub enum $E:ident {
                $(
                    $(#[$v_attr:meta])*
                    :$V:ident($by:expr) // The leading (ugly) colon is to suppress local ambiguity error.
                ),*;
                $(#[$u_attr:meta])*
                :$U:ident(_),
            }
        )*
    ) => {
        $(
            $(#[$attr])*
            pub enum $E {
                $(
                    $(#[$v_attr])*
                    $V,
                )*
                $(#[$u_attr])*
                $U(String),
            }

            impl<'x> ::serde::Deserialize<'x> for $E {
                fn deserialize<D: ::serde::Deserializer<'x>>(d: D) -> ::std::result::Result<Self, D::Error> {
                    struct V;

                    impl<'x> ::serde::de::Visitor<'x> for V {
                        type Value = $E;

                        fn visit_str<E>(self, s: &str) -> ::std::result::Result<$E, E> {
                            match s {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.to_owned())),
                            }
                        }

                        fn visit_string<E>(self, s: String) -> ::std::result::Result<$E, E> {
                            match s.as_str() {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s)),
                            }
                        }

                        fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                            write!(f, "a string")
                        }
                    }

                    d.deserialize_string(V)
                }
            }

            impl ::std::convert::AsRef<str> for $E {
                fn as_ref(&self) -> &str {
                    match *self {
                        $($E::$V => $by,)*
                        $E::$U(ref s) => s,
                    }
                }
            }

            impl ::std::cmp::PartialEq for $E {
                fn eq(&self, other: &$E) -> bool {
                    match *self {
                        $($E::$V => match *other {
                            $E::$V => true,
                            $E::$U(ref s) if $by == s => true,
                            _ => false,
                        },)*
                        $E::$U(ref s) => match *other {
                            $($E::$V => $by == s,)*
                            $E::$U(ref t) => s == t,
                        },
                    }
                }
            }

            impl ::std::hash::Hash for $E {
                fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                    match *self {
                        $($E::$V => $by.hash(state),)*
                        $E::$U(ref s) => s.hash(state),
                    }
                }
            }

            impl ::std::cmp::Eq for $E {}
        )*
    }
}

/// Non-blocking version of `futures::stream::iter()`.
pub fn iter_stream<T, E, I>(iter: I) -> IterStream<T, E>
    where T: 'static + Send, E: 'static + Send, I: 'static + IntoIterator<Item=Result<T, E>> + Send
{
    let (tx, rx) = mpsc::channel(8); // TODO: is this a proper value?

    thread::spawn(move || {
        let iter = iter.into_iter().map(Ok);
        let stream = stream::iter(iter);
        tx.send_all(stream).wait().is_ok() // Fails silently when `tx` is dropped.
    });

    fn thener<T_, E_>(r: Result<Result<T_, E_>, ()>) -> Result<T_, E_> {
        r.expect("Receiver failed")
    }

    rx.then(thener::<T, E> as _)
}

pub fn parse_datetime(s: &str) -> ::chrono::format::ParseResult<DateTime> {
    UTC.datetime_from_str(s, "%a %b %e %H:%M:%S %z %Y")
}

pub fn deserialize_datetime<'x, D: Deserializer<'x>>(d: D) -> Result<DateTime, D::Error> {
    parse_datetime(&String::deserialize(d)?).map_err(|e| D::Error::custom(e.to_string()))
}

impl ::std::error::Error for Never {
    fn description(&self) -> &str {
        unreachable!()
    }
}

impl Display for Never {
    fn fmt(&self, _: &mut Formatter) -> fmt::Result {
        unreachable!()
    }
}
