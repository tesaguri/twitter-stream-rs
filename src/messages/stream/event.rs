use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use super::super::DateTime;
use json::value::{Deserializer as JsonDeserializer, Value};
use super::{List, Tweet, User};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    created_at: DateTime,
    event: EventKind,
    target: User,
    source: User,
    target_object: Option<TargetObject>,
}

string_enums! {
    #[derive(Clone, Debug)]
    pub enum EventKind {
        AccessRevoked("access_revoked"),
        Block("block"),
        Unblock("unblock"),
        Favorite("favorite"),
        Unfavorite("unfavorite"),
        Follow("follow"),
        Unfollow("unfollow"),
        ListCreated("list_created"),
        ListDestroyed("list_destroyed"),
        ListUpdated("list_updated"),
        ListMemberAdded("list_member_added"),
        ListMemberRemoved("list_member_removed"),
        ListUserSubscribed("list_user_subscribed"),
        ListUserUnsubscribed("list_user_unsubscribed"),
        QuotedTweet("quoted_tweet"),
        UserUpdate("user_update");
        Unknown(_),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TargetObject {
    ClientApplication,
    Tweet(Tweet),
    List(List),
    Unknown(Value),
}

impl Deserialize for Event {
    fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
        struct EventVisitor;

        impl Visitor for EventVisitor {
            type Value = Event;

            fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<Event, V::Error> {
                use self::EventKind::*;

                #[derive(Default)]
                struct EventObject {
                    created_at: Option<DateTime>,
                    event: Option<EventKind>,
                    target: Option<User>,
                    source: Option<User>,
                    target_object: Option<Option<TargetObject>>,
                }

                let mut event = EventObject::default();
                let mut target_obj: Option<Value> = None;

                fn access_revoked_target(s: String) -> TargetObject {
                    match s.as_str() {
                        "client_application" => TargetObject::ClientApplication,
                        _ => TargetObject::Unknown(Value::String(s)),
                    }
                }

                macro_rules! err_map {
                    () => (|e| V::Error::custom(e.to_string()));
                }

                while let Some(k) = v.visit_key::<String>()? {
                    match k.as_str() {
                        "created_at" => {
                            let val = v.visit_value::<String>()?;
                            event.created_at = Some(
                                super::super::parse_datetime(&val).map_err(err_map!())?
                            );
                        },
                        "event" => {
                            let ek = v.visit_value()?;
                            if let Some(t) = target_obj.take() {
                                let mut d = JsonDeserializer::new(t);
                                event.target_object = match ek {
                                    AccessRevoked => Some(
                                        access_revoked_target(String::deserialize(&mut d).map_err(err_map!())?)
                                    ),
                                    Favorite | Unfavorite | QuotedTweet => Some(
                                        TargetObject::Tweet(Tweet::deserialize(&mut d).map_err(err_map!())?)
                                    ),
                                    ListCreated | ListDestroyed | ListUpdated | ListMemberAdded | ListMemberRemoved |
                                        ListUserSubscribed | ListUserUnsubscribed =>
                                    {
                                        Some(TargetObject::List(List::deserialize(&mut d).map_err(err_map!())?))
                                    },
                                    Block | Unblock | Follow | Unfollow | UserUpdate => {
                                        match Value::deserialize(&mut d).map_err(err_map!())? {
                                            Value::Null => None,
                                            val => Some(TargetObject::Unknown(val)),
                                        }
                                    },
                                    Unknown(_) => Some(
                                        TargetObject::Unknown(Value::deserialize(&mut d).map_err(err_map!())?)
                                    ),
                                }.into();
                            }
                            event.event = Some(ek);
                        },
                        "target" => event.target = Some(v.visit_value()?),
                        "source" => event.source = Some(v.visit_value()?),
                        "target_object" => {
                            if let Some(ref e) = event.event {
                                event.target_object = match *e {
                                    AccessRevoked => Some(access_revoked_target(v.visit_value()?)),
                                    Favorite | Unfavorite | QuotedTweet => Some(TargetObject::Tweet(v.visit_value()?)),
                                    ListCreated | ListDestroyed | ListUpdated | ListMemberAdded | ListMemberRemoved |
                                        ListUserSubscribed | ListUserUnsubscribed =>
                                    {
                                        Some(TargetObject::List(v.visit_value()?))
                                    },
                                    Block | Unblock | Follow | Unfollow | UserUpdate => {
                                        match v.visit_value()? {
                                            Value::Null => None,
                                            val => Some(TargetObject::Unknown(val)),
                                        }
                                    },
                                    Unknown(_) => Some(TargetObject::Unknown(v.visit_value()?)),
                                }.into();
                            } else {
                                target_obj = Some(v.visit_value()?);
                            }
                        },
                        _ => { v.visit_value::<IgnoredAny>()?; },
                    }
                }

                v.end()?;

                if let EventObject {
                        created_at: Some(ca), event: Some(ek), target: Some(t), source: Some(s), target_object: Some(to)
                    } = event
                {
                    Ok(Event {
                        created_at: ca,
                        event: ek,
                        target: t,
                        source: s,
                        target_object: to,
                    })
                } else {
                    v.missing_field(if event.created_at.is_none() {
                        "created_at"
                    } else if event.event.is_none() {
                        "event"
                    } else if event.target.is_none() {
                        "target"
                    } else if event.source.is_none() {
                        "source"
                    } else if event.target_object.is_none() {
                        "target_object"
                    } else {
                        unreachable!()
                    })
                }
            }
        }

        d.deserialize_map(EventVisitor)
    }
}
