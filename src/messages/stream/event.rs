use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use std::fmt;
use super::super::DateTime;
use json::Value;
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
    /// An object which indicates the name of the event and contains an optional object which
    /// represents the target of the event.
    pub event: EventKind,
    pub target: User,
    pub source: User,
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
            $Custom(String, Option<Value>),
        }

        impl Deserialize for Event {
            fn deserialize<D: Deserializer>(d: D) -> Result<Self, D::Error> {
                struct EventVisitor;

                impl Visitor for EventVisitor {
                    type Value = Event;

                    fn visit_map<V: MapVisitor>(self, mut v: V) -> Result<Event, V::Error> {
                        #[derive(Default)]
                        struct EventBuffer {
                            created_at: Option<DateTime>,
                            event: Option<EventKind>,
                            target: Option<User>,
                            source: Option<User>,
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
                                    event.event = if let Some(t) = target_obj.take() {
                                        match e.as_str() {
                                            $($c_tag => {
                                                $T::$Container(<$Content>::deserialize(t).map_err(err_map!())?)
                                            },)*
                                            $($l_tag => $T::$Label,)*
                                            _ => $T::$Custom(e, Some(t)),
                                        }.into()
                                    } else {
                                        match e.as_str() {
                                            $($l_tag => Some($T::$Label),)*
                                            $($c_tag)|* => { event_kind = Some(e); None },
                                            _ => Some($T::Custom(e, None)),
                                        }
                                    };
                                },
                                "target" => event.target = Some(v.visit_value()?),
                                "source" => event.source = Some(v.visit_value()?),
                                "target_object" => if let Some(e) = event_kind.take() {
                                    event.event = match e.as_str() {
                                        $($c_tag => $T::$Container(v.visit_value()?),)*
                                        $($l_tag => { v.visit_value::<IgnoredAny>()?; $T::$Label },)*
                                        _ => $T::$Custom(e, v.visit_value()?),
                                    }.into();
                                } else if event.event.is_none() {
                                    target_obj = Some(v.visit_value()?);
                                } else {
                                    v.visit_value::<IgnoredAny>()?;
                                },
                                _ => { v.visit_value::<IgnoredAny>()?; },
                            }

                            if let EventBuffer {
                                    created_at: Some(ca), event: Some(e), target: Some(t), source: Some(s),
                                } = event
                            {
                                while v.visit::<IgnoredAny, IgnoredAny>()?.is_some() {}
                                return Ok(Event { created_at: ca, event: e, target: t, source: s });
                            }
                        }

                        Err(V::Error::missing_field(if event.created_at.is_none() {
                            "created_at"
                        } else if event.target.is_none() {
                            "target"
                        } else if event.source.is_none() {
                            "source"
                        } else if event.event.is_none() {
                            if target_obj.is_some() {
                                "event"
                            } else {
                                "target_object"
                            }
                        } else {
                            unreachable!();
                        }))
                    }

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        write!(f, "a map")
                    }
                }

                d.deserialize_map(EventVisitor)
            }
        }
    };
}

impl_event! {
    /// An object which indicates the name of an event.
    /// It may contain an object called "target object" which represents the target of the event.
    ///
    /// The meaning of `target` and `source` field of an `Event` will be different based on the name of the event,
    /// as described below.
    ///
    /// | Description                         | Event Name             | `source`           | `target`       |
    /// | ----------------------------------- | ---------------------- | ------------------ | -------------- |
    /// | User deauthorizes stream            | `AccessRevoked`        | Deauthorizing user | App owner      |
    /// | User blocks someone                 | `Block`                | Current user       | Blocked user   |
    /// | User removes a block                | `Unblock`              | Current user       | Unblocked user |
    /// | User favorites a Tweet              | `Favorite`             | Current user       | Tweet author   |
    /// | User's Tweet is favorited           | `Favorite`             | Favoriting user    | Current user   |
    /// | User unfavorites a Tweet            | `Unfavorite`           | Current user       | Tweet author   |
    /// | User's Tweet is unfavorited         | `Unfavorite`           | Unfavoriting user  | Current user   |
    /// | User follows someone                | `Follow`               | Current user       | Followed user  |
    /// | User is followed                    | `Follow`               | Following user     | Current user   |
    /// | User unfollows someone              | `Unfollow`             | Current user       | Followed user  |
    /// | User creates a list                 | `ListCreated`          | Current user       | Current user   |
    /// | User deletes a list                 | `ListDestroyed`        | Current user       | Current user   |
    /// | User edits a list                   | `ListUpdated`          | Current user       | Current user   |
    /// | User adds someone to a list         | `ListMemberAdded`      | Current user       | Added user     |
    /// | User is added to a list             | `ListMemberAdded`      | Adding user        | Current user   |
    /// | User removes someone from a list    | `ListMemberRemoved`    | Current user       | Removed user   |
    /// | User is removed from a list         | `ListMemberRemoved`    | Removing user      | Current user   |
    /// | User subscribes to a list           | `ListUserSubscribed`   | Current user       | List owner     |
    /// | User's list is subscribed to        | `ListUserSubscribed`   | Subscribing user   | Current user   |
    /// | User unsubscribes from a list       | `ListUserUnsubscribed` | Current user       | List owner     |
    /// | User's list is unsubscribed from    | `ListUserUnsubscribed` | Unsubscribing user | Current user   |
    /// | User's Tweet is quoted              | `QuotedTweet`          | quoting User       | Current User   |
    /// | User updates their profile          | `UserUpdate`           | Current user       | Current user   |
    /// | User updates their protected status | `UserUpdate`           | Current user       | Current user   |
    #[derive(Clone, Debug, PartialEq)]
    pub enum EventKind {
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
        /// An event this library does not know. The first value is raw event name
        /// and the second is the target object.
        :Custom(_, _),
    }
}
