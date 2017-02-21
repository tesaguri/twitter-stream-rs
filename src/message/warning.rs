use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use std::fmt;
use user::UserId;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Warning {
    pub message: String,
    pub code: WarningCode,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum WarningCode {
    FallingBehind(u64),
    FollowsOverLimit(UserId),
    Custom(String),
}

impl Deserialize for Warning {
    fn deserialize<D: Deserializer>(d: D) -> Result<Self, D::Error> {
        struct WarningVisitor;

        impl Visitor for WarningVisitor {
            type Value = Warning;

            fn visit_map<V: MapVisitor>(self, mut v: V) -> Result<Warning, V::Error> {
                string_enums! {
                    pub enum Code {
                        :FallingBehind("FALLING_BEHIND"),
                        :FollowsOverLimit("FOLLOWS_OVER_LIMIT");
                        :Custom(_),
                    }
                }

                let mut code = None;
                let mut message: Option<String> = None;
                let mut percent_full: Option<u64> = None;
                let mut user_id: Option<UserId> = None;

                while let Some(k) = v.visit_key::<String>()? {
                    match k.as_str() {
                        "code" => code = Some(v.visit_value::<Code>()?),
                        "message" => message = Some(v.visit_value()?),
                        "percent_full" => percent_full = Some(v.visit_value()?),
                        "user_id" => user_id = Some(v.visit_value()?),
                        _ => { v.visit_value::<IgnoredAny>()?; },
                    }

                    macro_rules! end {
                        () => {{
                            while v.visit::<IgnoredAny,IgnoredAny>()?.is_some() {}
                        }};
                    }

                    match (code.as_ref(), message.as_ref(), percent_full, user_id) {
                        (Some(&Code::FallingBehind), Some(_), Some(percent_full), _) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap(),
                                code: WarningCode::FallingBehind(percent_full),
                            });
                        },
                        (Some(&Code::FollowsOverLimit), Some(_), _, Some(user_id)) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap(),
                                code: WarningCode::FollowsOverLimit(user_id),
                            });
                        },
                        (Some(&Code::Custom(_)), Some(_), _, _) => {
                            end!();
                            if let Some(Code::Custom(code)) = code {
                                return Ok(Warning {
                                    message: message.unwrap(),
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
                    Err(V::Error::missing_field("code"))
                } else if message.is_none() {
                    Err(V::Error::missing_field("message"))
                } else if code == Some(Code::FallingBehind) {
                    Err(V::Error::missing_field("percent_full"))
                } else {
                    Err(V::Error::missing_field("user_id"))
                }
            }

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map")
            }
        }

        d.deserialize_map(WarningVisitor)
    }
}
