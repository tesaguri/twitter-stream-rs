use serde::de::{Deserialize, Deserializer, Error as SerdeError};
use types::DateTime;

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
    parse_datetime(&String::deserialize(d)?).map_err(|e| D::Error::custom(e.to_string()))
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
