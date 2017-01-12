use serde::de::{Deserialize, Deserializer, Error, MapVisitor, Visitor};
use serde::de::impls::IgnoredAny;
use super::super::DateTime;
use json::value::{Deserializer as JsonDeserializer, Value};
use super::{List, Tweet, User};

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub created_at: DateTime,
    pub event: EventKind,
    pub target: User,
    pub source: User,
    pub target_object: Option<TargetObject>,
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
        Custom(_),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TargetObject {
    ClientApplication,
    Tweet(Tweet),
    List(List),
    Custom(Value),
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
                        _ => TargetObject::Custom(Value::String(s)),
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
                            event.target_object = if let Some(t) = target_obj.take() {
                                let mut d = JsonDeserializer::new(t);
                                match ek {
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
                                            val => Some(TargetObject::Custom(val)),
                                        }
                                    },
                                    Custom(_) => Some(
                                        TargetObject::Custom(Value::deserialize(&mut d).map_err(err_map!())?)
                                    ),
                                }.into()
                            } else {
                                match ek {
                                    Block | Unblock | Follow | Unfollow | UserUpdate | Custom(_) => Some(None),
                                    _ => None,
                                }
                            };
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
                                            val => Some(TargetObject::Custom(val)),
                                        }
                                    },
                                    Custom(_) => Some(TargetObject::Custom(v.visit_value()?)),
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, UTC};
    use json;
    use super::*;

    #[test]
    fn deserialize() {
        assert_eq!(
            Event {
                created_at: UTC.ymd(2016, 12, 26).and_hms(0, 39, 13),
                event: EventKind::Follow,
                source: User {
                    id: 783214,
                    // id_str: "783214",
                    name: "Twitter".to_owned(),
                    screen_name: "Twitter".to_owned(),
                    location: Some("San Francisco, CA".to_owned()),
                    description: Some(
                        "Your official source for news, updates, and tips from Twitter, Inc. Need help? \
                            Visit https://t.co/jTMg7YsLw5.".to_owned()
                    ),
                    url: Some("https://t.co/gN5JJwhQy7".to_owned()),
                    protected: false,
                    followers_count: 58213762,
                    friends_count: 155,
                    listed_count: 90496,
                    created_at: UTC.ymd(2007, 2, 20).and_hms(14, 35, 54),
                    favourites_count: 3003,
                    utc_offset: Some(-28800),
                    time_zone: Some("Pacific Time (US & Canada)".to_owned()),
                    geo_enabled: true,
                    verified: true,
                    statuses_count: 3695,
                    lang: "en".to_owned(),
                    contributors_enabled: false,
                    is_translator: false,
                    // is_translation_enabled: false,
                    profile_background_color: "ACDED6".to_owned(),
                    profile_background_image_url:
                        "http://pbs.twimg.com/profile_background_images/657090062/l1uqey5sy82r9ijhke1i.png".to_owned(),
                    profile_background_image_url_https:
                        "https://pbs.twimg.com/profile_background_images/657090062/l1uqey5sy82r9ijhke1i.png".to_owned(),
                    profile_background_tile: true,
                    profile_image_url:
                        "http://pbs.twimg.com/profile_images/767879603977191425/29zfZY6I_normal.jpg".to_owned(),
                    profile_image_url_https:
                        "https://pbs.twimg.com/profile_images/767879603977191425/29zfZY6I_normal.jpg".to_owned(),
                    profile_banner_url: Some("https://pbs.twimg.com/profile_banners/783214/1476219753".to_owned()),
                    profile_link_color: "226699".to_owned(),
                    profile_sidebar_border_color: "FFFFFF".to_owned(),
                    profile_sidebar_fill_color: "F6F6F6".to_owned(),
                    profile_text_color: "333333".to_owned(),
                    profile_use_background_image: true,
                    default_profile: false,
                    default_profile_image: false,
                    // following: false,
                    follow_request_sent: Some(false),
                    // notifications: false,
                    // translator_type: "regular".to_owned(),
                    withheld_in_countries: None,
                    withheld_scope: None,
                },
                target: User {
                    id: 12,
                    // id_str: "12".to_owned(),
                    name: "jack".to_owned(),
                    screen_name: "jack".to_owned(),
                    location: Some("California, USA".to_owned()),
                    description: Some("".to_owned()),
                    url: None,
                    protected: false,
                    followers_count: 3949461,
                    friends_count: 2379,
                    listed_count: 26918,
                    created_at: UTC.ymd(2006, 3, 21).and_hms(20, 50, 14),
                    favourites_count: 15494,
                    utc_offset: Some(-28800),
                    time_zone: Some("Pacific Time (US & Canada)".to_owned()),
                    geo_enabled: true,
                    verified: true,
                    statuses_count: 21033,
                    lang: "en".to_owned(),
                    contributors_enabled: false,
                    is_translator: false,
                    // is_translation_enabled: false,
                    profile_background_color: "EBEBEB".to_owned(),
                    profile_background_image_url: "http://abs.twimg.com/images/themes/theme7/bg.gif".to_owned(),
                    profile_background_image_url_https: "https://abs.twimg.com/images/themes/theme7/bg.gif".to_owned(),
                    profile_background_tile: false,
                    profile_image_url:
                        "http://pbs.twimg.com/profile_images/768529565966667776/WScYY_cq_normal.jpg".to_owned(),
                    profile_image_url_https:
                        "https://pbs.twimg.com/profile_images/768529565966667776/WScYY_cq_normal.jpg".to_owned(),
                    profile_banner_url: Some("https://pbs.twimg.com/profile_banners/12/1483046077".to_owned()),
                    profile_link_color: "990000".to_owned(),
                    profile_sidebar_border_color: "DFDFDF".to_owned(),
                    profile_sidebar_fill_color: "F3F3F3".to_owned(),
                    profile_text_color: "333333".to_owned(),
                    profile_use_background_image: true,
                    default_profile: false,
                    default_profile_image: false,
                    // following: None,
                    follow_request_sent: Some(false),
                    // notifications: false,
                    // translator_type: "regular".to_owned(),
                    withheld_in_countries: None,
                    withheld_scope: None,
                },
                target_object: None,
            },
            json::from_str::<Event>("{
                \"created_at\":\"Mon Dec 26 00:39:13 +0000 2016\",
                \"event\":\"follow\",
                \"source\": {
                    \"id\": 783214,
                    \"id_str\": \"783214\",
                    \"name\": \"Twitter\",
                    \"screen_name\": \"Twitter\",
                    \"location\": \"San Francisco, CA\",
                    \"description\": \"Your official source for news, updates, and tips from Twitter, Inc. \
                        Need help? Visit https://t.co/jTMg7YsLw5.\",
                    \"url\": \"https://t.co/gN5JJwhQy7\",
                    \"protected\": false,
                    \"followers_count\": 58213762,
                    \"friends_count\": 155,
                    \"listed_count\": 90496,
                    \"created_at\": \"Tue Feb 20 14:35:54 +0000 2007\",
                    \"favourites_count\": 3003,
                    \"utc_offset\": -28800,
                    \"time_zone\": \"Pacific Time (US & Canada)\",
                    \"geo_enabled\": true,
                    \"verified\": true,
                    \"statuses_count\": 3695,
                    \"lang\": \"en\",
                    \"contributors_enabled\": false,
                    \"is_translator\": false,
                    \"is_translation_enabled\": false,
                    \"profile_background_color\": \"ACDED6\",
                    \"profile_background_image_url\":
                        \"http://pbs.twimg.com/profile_background_images/657090062/l1uqey5sy82r9ijhke1i.png\",
                    \"profile_background_image_url_https\":
                        \"https://pbs.twimg.com/profile_background_images/657090062/l1uqey5sy82r9ijhke1i.png\",
                    \"profile_background_tile\": true,
                    \"profile_image_url\":
                        \"http://pbs.twimg.com/profile_images/767879603977191425/29zfZY6I_normal.jpg\",
                    \"profile_image_url_https\":
                        \"https://pbs.twimg.com/profile_images/767879603977191425/29zfZY6I_normal.jpg\",
                    \"profile_banner_url\": \"https://pbs.twimg.com/profile_banners/783214/1476219753\",
                    \"profile_link_color\": \"226699\",
                    \"profile_sidebar_border_color\": \"FFFFFF\",
                    \"profile_sidebar_fill_color\": \"F6F6F6\",
                    \"profile_text_color\": \"333333\",
                    \"profile_use_background_image\": true,
                    \"default_profile\": false,
                    \"default_profile_image\": false,
                    \"following\": null,
                    \"follow_request_sent\": false,
                    \"notifications\": false,
                    \"translator_type\": \"regular\"
                },
                \"target\": {
                    \"id\": 12,
                    \"id_str\": \"12\",
                    \"name\": \"jack\",
                    \"screen_name\": \"jack\",
                    \"location\": \"California, USA\",
                    \"description\": \"\",
                    \"url\": null,
                    \"protected\": false,
                    \"followers_count\": 3949461,
                    \"friends_count\": 2379,
                    \"listed_count\": 26918,
                    \"created_at\": \"Tue Mar 21 20:50:14 +0000 2006\",
                    \"favourites_count\": 15494,
                    \"utc_offset\": -28800,
                    \"time_zone\": \"Pacific Time (US & Canada)\",
                    \"geo_enabled\": true,
                    \"verified\": true,
                    \"statuses_count\": 21033,
                    \"lang\": \"en\",
                    \"contributors_enabled\": false,
                    \"is_translator\": false,
                    \"is_translation_enabled\": false,
                    \"profile_background_color\": \"EBEBEB\",
                    \"profile_background_image_url\": \"http://abs.twimg.com/images/themes/theme7/bg.gif\",
                    \"profile_background_image_url_https\": \"https://abs.twimg.com/images/themes/theme7/bg.gif\",
                    \"profile_background_tile\": false,
                    \"profile_image_url\":
                        \"http://pbs.twimg.com/profile_images/768529565966667776/WScYY_cq_normal.jpg\",
                    \"profile_image_url_https\":
                        \"https://pbs.twimg.com/profile_images/768529565966667776/WScYY_cq_normal.jpg\",
                    \"profile_banner_url\": \"https://pbs.twimg.com/profile_banners/12/1483046077\",
                    \"profile_link_color\": \"990000\",
                    \"profile_sidebar_border_color\": \"DFDFDF\",
                    \"profile_sidebar_fill_color\": \"F3F3F3\",
                    \"profile_text_color\": \"333333\",
                    \"profile_use_background_image\": true,
                    \"default_profile\": false,
                    \"default_profile_image\": false,
                    \"following\": null,
                    \"follow_request_sent\": false,
                    \"notifications\": false,
                    \"translator_type\": \"regular\"
                }
            }").unwrap()
        );
    }
}
