use serde::de::{Deserialize, Deserializer, Error, IgnoredAny, MapAccess, Visitor};
use std::borrow::Cow;
use std::fmt;
use user::UserId;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Warning<'a> {
    pub message: Cow<'a, str>,
    pub code: WarningCode<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum WarningCode<'a> {
    FallingBehind(u64),
    FollowsOverLimit(UserId),
    Custom(Cow<'a, str>),
}

impl<'de: 'a, 'a> Deserialize<'de> for Warning<'a> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct WarningVisitor;

        impl<'a> Visitor<'a> for WarningVisitor {
            type Value = Warning<'a>;

            fn visit_map<A: MapAccess<'a>>(self, mut a: A) -> Result<Warning<'a>, A::Error> {
                use util::CowStr;

                string_enums! {
                    pub enum Code<'a> {
                        :FallingBehind("FALLING_BEHIND"),
                        :FollowsOverLimit("FOLLOWS_OVER_LIMIT");
                        :Custom(_),
                    }
                }

                let mut code = None;
                let mut message: Option<CowStr> = None;
                let mut percent_full: Option<u64> = None;
                let mut user_id: Option<UserId> = None;

                while let Some(k) = a.next_key::<CowStr>()? {
                    match k.as_ref() {
                        "code" => code = Some(a.next_value::<Code>()?),
                        "message" => message = Some(a.next_value()?),
                        "percent_full" => percent_full = Some(a.next_value()?),
                        "user_id" => user_id = Some(a.next_value()?),
                        _ => { a.next_value::<IgnoredAny>()?; },
                    }

                    macro_rules! end {
                        () => {{
                            while a.next_entry::<IgnoredAny,IgnoredAny>()?.is_some() {}
                        }};
                    }

                    match (code.as_ref(), message.as_ref(), percent_full, user_id) {
                        (Some(&Code::FallingBehind), Some(_), Some(percent_full), _) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap().0,
                                code: WarningCode::FallingBehind(percent_full),
                            });
                        },
                        (Some(&Code::FollowsOverLimit), Some(_), _, Some(user_id)) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap().0,
                                code: WarningCode::FollowsOverLimit(user_id),
                            });
                        },
                        (Some(&Code::Custom(_)), Some(_), _, _) => {
                            end!();
                            if let Some(Code::Custom(code)) = code {
                                return Ok(Warning {
                                    message: message.unwrap().0,
                                    code: WarningCode::Custom(code),
                                });
                            } else {
                                unreachable!();
                            }
                        },
                        _ => (),
                    }
                }

                if code.is_none() {
                    Err(A::Error::missing_field("code"))
                } else if message.is_none() {
                    Err(A::Error::missing_field("message"))
                } else if code == Some(Code::FallingBehind) {
                    Err(A::Error::missing_field("percent_full"))
                } else {
                    Err(A::Error::missing_field("user_id"))
                }
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map")
            }
        }

        d.deserialize_map(WarningVisitor)
    }
}
