use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use super::super::DateTime;
use json::value::{Deserializer as JsonDeserializer, Value};
use super::{List, Tweet, User};

/// Represents notifications about non-Tweet events are also sent over a stream.
///
/// # Reference
///
/// 1. [Streaming message types — Twitter Developers]
///    (https://dev.twitter.com/streaming/overview/messages-types#Events_event)
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub created_at: DateTime,
    pub target: User,
    pub source: User,
    pub target_object: TargetObject,
}

macro_rules! impl_event {
    (
        $(#[$attr:meta])*
        pub enum $T:ident {
            $(
                $(#[$c_attr:meta])*
                :$Container:ident($c_tag:expr, $Content:ty)
            ),*;
            $(
                $(#[$l_attr:meta])*
                :$Label:ident($l_tag:expr)
            ),*;
            $(#[$cu_attr:meta])*
            :$Custom:ident(_, _),
        }
    ) => {
        $(#[$attr])*
        pub enum $T {
            $(
                $(#[$c_attr])*
                $Container($Content),
            )*
            $(
                $(#[$l_attr])*
                $Label,
            )*
            $(#[$cu_attr])*
            $Custom(String, Value),
        }

        impl Deserialize for Event {
            fn deserialize<D: Deserializer>(d: &mut D) -> Result<Self, D::Error> {
                struct EventVisitor;

                impl Visitor for EventVisitor {
                    type Value = Event;

                    fn visit_map<V: MapVisitor>(&mut self, mut v: V) -> Result<Event, V::Error> {
                        #[derive(Default)]
                        struct EventBuffer {
                            created_at: Option<DateTime>,
                            target: Option<User>,
                            source: Option<User>,
                            target_object: Option<TargetObject>,
                        }

                        let mut event = EventBuffer::default();
                        let mut event_kind: Option<String> = None;
                        let mut target_obj: Option<Value> = None;

                        macro_rules! err_map {
                            () => (|e| V::Error::custom(e.to_string()));
                        }

                        while let Some(k) = v.visit_key::<String>()? {
                            match k.as_str() {
                                "created_at" => {
                                    let val = v.visit_value::<String>()?;
                                    event.created_at = Some(super::super::parse_datetime(&val).map_err(err_map!())?);
                                },
                                "event" => {
                                    let e = v.visit_value::<String>()?;
                                    event.target_object = if let Some(t) = target_obj.take() {
                                        match e.as_str() {
                                            $($c_tag => {
                                                let mut d = JsonDeserializer::new(t);
                                                $T::$Container(<$Content>::deserialize(&mut d).map_err(err_map!())?)
                                            },)*
                                            $($l_tag => $T::$Label,)*
                                            _ => $T::$Custom(e, t),
                                        }.into()
                                    } else {
                                        match e.as_str() {
                                            $($l_tag => Some($T::$Label),)*
                                            _ => { event_kind = Some(e); None },
                                        }
                                    };
                                },
                                "target" => event.target = Some(v.visit_value()?),
                                "source" => event.source = Some(v.visit_value()?),
                                "target_object" => if let Some(e) = event_kind.take() {
                                    event.target_object = match e.as_str() {
                                        $($c_tag => $T::$Container(v.visit_value()?),)*
                                        $($l_tag => { v.visit_value::<IgnoredAny>()?; $T::$Label },)*
                                        _ => $T::$Custom(e, v.visit_value()?),
                                    }.into();
                                } else if event.target_object.is_none() {
                                    target_obj = Some(v.visit_value()?);
                                } else {
                                    v.visit_value::<IgnoredAny>()?;
                                },
                                _ => { v.visit_value()?; },
                            }

                            if let EventBuffer {
                                    created_at: Some(ca), target: Some(t), source: Some(s), target_object: Some(to),
                                } = event
                            {
                                while let Some(_) = v.visit::<IgnoredAny, IgnoredAny>()? {}
                                v.end()?;
                                return Ok(Event { created_at: ca, target: t, source: s, target_object: to });
                            }
                        }

                        v.end()?;

                        v.missing_field(if event.created_at.is_none() {
                            "created_at"
                        } else if event.target.is_none() {
                            "target"
                        } else if event.source.is_none() {
                            "source"
                        } else if event.target_object.is_none() {
                            if target_obj.is_some() {
                                "event"
                            } else {
                                "target_object"
                            }
                        } else {
                            unreachable!();
                        })
                    }
                }

                d.deserialize_map(EventVisitor)
            }
        }
    };
}

impl_event! {
    #[derive(Clone, Debug, PartialEq)]
    pub enum TargetObject {
        :Favorite("favorite", Tweet),
        :Unfavorite("unfavorite", Tweet),
        :ListCreated("list_created", List),
        :ListDestroyed("list_destroyed", List),
        :ListUpdated("list_updated", List),
        :ListMemberAdded("list_member_added", List),
        :ListMemberRemoved("list_member_removed", List),
        :ListUserSubscribed("list_user_subscribed", List),
        :ListUserUnsubscribed("list_user_unsubscribed", List),
        :QuotedTweet("quoted_tweet", Tweet);
        :AccessRevoked("access_revoked"),
        :Block("block"),
        :Unblock("unblock"),
        :Follow("follow"),
        :Unfollow("unfollow"),
        :UserUpdate("user_update");
        :Custom(_, _),
    }
}
