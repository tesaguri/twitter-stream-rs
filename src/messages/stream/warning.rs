use serde::de::{Deserialize, Deserializer, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use super::UserId;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Warning {
    pub message: String,
    pub code: WarningCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WarningCode {
    FallingBehind(u64),
    FollowsOverLimit(UserId),
    Unknown,
}

impl Deserialize for Warning {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        struct WarningVisitor;

        impl Visitor for WarningVisitor {
            type Value = Warning;

            fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<Warning, V::Error> {
                #[derive(Clone, Copy, Deserialize, PartialEq)]
                enum Code {
                    FallingBehind,
                    FollowsOverLimit,
                    Unknown,
                }

                let mut code = None;
                let mut message: Option<String> = None;
                let mut percent_full: Option<u64> = None;
                let mut user_id: Option<UserId> = None;

                while let Some(k) = v.visit_key::<String>()? {
                    match k.as_str() {
                        "code" => {
                            let c = v.visit_value::<String>()?;
                            code = match c.as_str() {
                                "FALLING_BEHIND" => Code::FallingBehind,
                                "FOLLOWS_OVER_LIMIT" => Code::FollowsOverLimit,
                                _ => Code::Unknown,
                            }.into();
                        },
                        "message" => message = Some(v.visit_value()?),
                        "percent_full" => percent_full = Some(v.visit_value()?),
                        "user_id" => user_id = Some(v.visit_value()?),
                        _ => { v.visit_value::<IgnoredAny>()?; },
                    }

                    macro_rules! end {
                        () => {{
                            while let Some(_) = v.visit::<IgnoredAny,IgnoredAny>()? {}
                            v.end()?;
                        }};
                    }

                    match (code, message.as_ref(), percent_full, user_id) {
                        (Some(Code::FallingBehind), Some(_), Some(percent_full), _) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap(),
                                code: WarningCode::FallingBehind(percent_full),
                            });
                        },
                        (Some(Code::FollowsOverLimit), Some(_), _, Some(user_id)) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap(),
                                code: WarningCode::FollowsOverLimit(user_id),
                            });
                        },
                        (Some(Code::Unknown), Some(_), _, _) => {
                            end!();
                            return Ok(Warning {
                                message: message.unwrap(),
                                code: WarningCode::Unknown,
                            });
                        },
                        _ => (),
                    }
                }

                if code.is_none() {
                    v.missing_field("code")
                } else if message.is_none() {
                    v.missing_field("message")
                } else if code == Some(Code::FallingBehind) {
                    v.missing_field("percent_full")
                } else {
                    v.missing_field("user_id")
                }
            }
        }

        d.deserialize_map(WarningVisitor)
    }
}
