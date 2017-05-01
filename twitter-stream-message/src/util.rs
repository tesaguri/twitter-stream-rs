use serde::de::{Deserialize, DeserializeSeed, Deserializer, Error as SerdeError, IntoDeserializer, MapAccess, Visitor};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{self, Formatter};
use std::mem;
use types::{DateTime, JsonValue};

macro_rules! string_enums {
    (
        $(
            $(#[$attr:meta])*
            pub enum $E:ident<$lifetime:tt> {
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
            pub enum $E<$lifetime> {
                $(
                    $(#[$v_attr])*
                    $V,
                )*
                $(#[$u_attr])*
                $U(::std::borrow::Cow<$lifetime, str>),
            }

            impl<'de: 'a, 'a> ::serde::Deserialize<'de> for $E<'a> {
                fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> ::std::result::Result<Self, D::Error> {
                    struct V;

                    impl<'a> ::serde::de::Visitor<'a> for V {
                        type Value = $E<'a>;

                        fn visit_str<E>(self, s: &str) -> ::std::result::Result<$E<'a>, E> {
                            match s {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.to_owned().into())),
                            }
                        }

                        fn visit_borrowed_str<E>(self, s: &'a str) -> ::std::result::Result<$E<'a>, E> {
                            match s {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.into())),
                            }
                        }

                        fn visit_string<E>(self, s: String) -> ::std::result::Result<$E<'a>, E> {
                            match s.as_str() {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.into())),
                            }
                        }

                        fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                            write!(f, "a string")
                        }
                    }

                    d.deserialize_str(V)
                }
            }

            impl<'a> ::std::convert::AsRef<str> for $E<'a> {
                fn as_ref(&self) -> &str {
                    match *self {
                        $($E::$V => $by,)*
                        $E::$U(ref s) => s.as_ref(),
                    }
                }
            }

            impl<'a> ::std::cmp::PartialEq for $E<'a> {
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

            impl<'a> ::std::hash::Hash for $E<'a> {
                fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                    match *self {
                        $($E::$V => $by.hash(state),)*
                        $E::$U(ref s) => s.hash(state),
                    }
                }
            }

            impl<'a> ::std::cmp::Eq for $E<'a> {}
        )*
    }
}

#[derive(Deserialize, Eq, PartialEq, Hash)]
pub struct CowStr<'a>(
    #[serde(borrow)]
    pub Cow<'a, str>
);

/// A `MapAccess` that first yields values from `keys` and `vals`, and after they are exhausted, yields from `tail`.
pub struct MapAccessChain<I, J, A> where I: IntoIterator, J: IntoIterator {
    keys: I::IntoIter,
    vals: J::IntoIter,
    tail: A,
}

impl<'a> AsRef<str> for CowStr<'a> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<I, J, A> MapAccessChain<I, J, A> where I: IntoIterator, J: IntoIterator {
    pub fn new(keys: I, vals: J, a: A) -> MapAccessChain<I, J, A> {
        MapAccessChain {
            keys: keys.into_iter(),
            vals: vals.into_iter(),
            tail: a,
        }
    }
}

impl<'de: 'a, 'a, I, J, A> MapAccess<'de> for MapAccessChain<I, J, A>
    where I: IntoIterator<Item=Cow<'a, str>>, J: IntoIterator<Item=JsonValue>, A: MapAccess<'de>
{
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, A::Error> {
        if let Some(k) = self.keys.next() {
            seed.deserialize(k.into_deserializer()).map(Some)
        } else {
            self.tail.next_key_seed(seed)
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, A::Error> {
        if let Some(v) = self.vals.next() {
            seed.deserialize(v).map_err(A::Error::custom)
        } else {
            self.tail.next_value::<JsonValue>().and_then(|v| {
                seed.deserialize(v).map_err(A::Error::custom)
            })

            // FIXME: The above code unnecessarily copies data from the input. Initially, I wrote that like below:
            // `self.tail.next_value_seed(seed)`
            // ... but this somehow caused `message::tests::parse` test to fail, with the error being as following:
            // `ErrorImpl { code: ExpectedObjectCommaOrEnd, line: 67, column: 38 }`
        }
    }
}

pub fn deserialize_default<'de, D, T>(d: D) -> Result<T, D::Error>
    where D: Deserializer<'de>, T: Default + Deserialize<'de>
{
    Option::deserialize(d).map(|o| o.unwrap_or_else(T::default))
}

pub fn parse_datetime(s: &str) -> ::chrono::format::ParseResult<DateTime> {
    use chrono::UTC;
    use chrono::format::{self, Fixed, Item, Numeric, Pad, Parsed};

    // "%a %b %e %H:%M:%S %z %Y"
    const ITEMS: &'static [Item<'static>] = &[
        Item::Fixed(Fixed::ShortWeekdayName),
        Item::Space(" "),
        Item::Fixed(Fixed::ShortMonthName),
        Item::Space(" "),
        Item::Numeric(Numeric::Day, Pad::Space),
        Item::Space(" "),
        Item::Numeric(Numeric::Hour, Pad::Zero),
        Item::Literal(":"),
        Item::Numeric(Numeric::Minute, Pad::Zero),
        Item::Literal(":"),
        Item::Numeric(Numeric::Second, Pad::Zero),
        Item::Space(" "),
        Item::Fixed(Fixed::TimezoneOffset),
        Item::Space(" "),
        Item::Numeric(Numeric::Year, Pad::Zero),
    ];

    let mut parsed = Parsed::new();
    format::parse(&mut parsed, s, ITEMS.iter().cloned())?;
    parsed.to_datetime_with_timezone(&UTC)
}

pub fn deserialize_datetime<'x, D: Deserializer<'x>>(d: D) -> Result<DateTime, D::Error> {
    struct DTVisitor;

    impl<'x> Visitor<'x> for DTVisitor {
        type Value = DateTime;

        fn visit_str<E: SerdeError>(self, s: &str) -> Result<DateTime, E> {
            parse_datetime(s).map_err(|e| E::custom(e.to_string()))
        }

        fn expecting(&self, f: &mut Formatter) -> fmt::Result {
            write!(f, "a formatted date and time string")
        }
    }

    d.deserialize_str(DTVisitor)
}

/// Deserializes a map of strings in zero-copy fashion.
pub fn deserialize_map_cow_str<'de: 'a, 'a, D>(d: D) -> Result<HashMap<Cow<'a, str>, Cow<'a, str>>, D::Error>
    where D: Deserializer<'de>
{
    #[derive(Deserialize)]
    struct MapDeserialize<'a>(
        #[serde(borrow)]
        HashMap<CowStr<'a>, CowStr<'a>>
    );

    MapDeserialize::deserialize(d).map(|m| unsafe {
        mem::transmute::<MapDeserialize<'a>, HashMap<Cow<'a, str>, Cow<'a, str>>>(m)
    }).map_err(D::Error::custom)
}

/// Deserializes an optional string in zero-copy fashion.
pub fn deserialize_opt_cow_str<'de: 'a, 'a, D: Deserializer<'de>>(d: D) -> Result<Option<Cow<'a, str>>, D::Error> {
    #[derive(Deserialize)]
    struct OptDeserialize<'a>(
        #[serde(borrow)]
        Option<CowStr<'a>>
    );

    OptDeserialize::deserialize(d).map(|o| unsafe {
        mem::transmute::<OptDeserialize<'a>, Option<Cow<'a, str>>>(o)
    }).map_err(D::Error::custom)
}

/// Deserializes a sequenve of strings in zero-copy fashion.
pub fn deserialize_vec_cow_str<'de: 'a, 'a, D: Deserializer<'de>>(d: D) -> Result<Vec<Cow<'a, str>>, D::Error> {
    #[derive(Deserialize)]
    struct VecDeserialize<'a>(
        #[serde(borrow)]
        Vec<CowStr<'a>>
    );

    VecDeserialize::deserialize(d).map(|v| unsafe {
        mem::transmute::<VecDeserialize<'a>, Vec<Cow<'a, str>>>(v)
    }).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_default() {
        use json;

        #[derive(Debug, Default, Deserialize, PartialEq)]
        struct S {
            #[serde(deserialize_with = "deserialize_default")]
            #[serde(default)]
            n: u32,
            #[serde(deserialize_with = "deserialize_default")]
            #[serde(default)]
            o: Option<bool>,
            #[serde(deserialize_with = "deserialize_default")]
            #[serde(default)]
            s: String,
            #[serde(deserialize_with = "deserialize_default")]
            #[serde(default)]
            v: Vec<u8>,
        }

        assert_eq!(
            json::from_str::<S>(r#"{"n":null,"s":null}"#).unwrap(),
            json::from_str(r#"{"o":null,"v":null}"#).unwrap()
        );
        assert_eq!(
            S { n: 1, o: Some(true), s: "s".to_owned(), v: vec![255] },
            json::from_str(r#"{"n":1,"o":true,"s":"s","v":[255]}"#).unwrap()
        );
    }

    #[test]
    fn test_parse_datetime() {
        use chrono::{NaiveDate, UTC};
        use types::DateTime;

        assert_eq!(
            DateTime::from_utc(NaiveDate::from_ymd(2017, 5, 1).and_hms(0, 1, 2), UTC),
            parse_datetime("Mon May 01 00:01:02 +0000 2017").unwrap()
        );
        assert!(parse_datetime("2017-05-01T00:01:02Z").is_err());
    }
}
