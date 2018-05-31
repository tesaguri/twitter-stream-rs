use std::borrow::Cow;

/// An OAuth token used to log into Twitter.
#[cfg_attr(feature = "tweetust", doc = "

This implements `tweetust::conn::Authenticator` so you can pass it to
`tweetust::TwitterClient` as if it were `tweetust::OAuthAuthenticator`"
)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[derive(Clone, Debug)]
pub struct Token<'a> {
    pub consumer_key: Cow<'a, str>,
    pub consumer_secret: Cow<'a, str>,
    pub access_key: Cow<'a, str>,
    pub access_secret: Cow<'a, str>,
}

impl<'a> Token<'a> {
    pub fn new<CK, CS, AK, AS>(
        consumer_key: CK,
        consumer_secret: CS,
        access_key: AK,
        access_secret: AS
    )
        -> Self
    where
        CK: Into<Cow<'a, str>>,
        CS: Into<Cow<'a, str>>,
        AK: Into<Cow<'a, str>>,
        AS: Into<Cow<'a, str>>,
    {
        Token {
            consumer_key: consumer_key.into(),
            consumer_secret: consumer_secret.into(),
            access_key: access_key.into(),
            access_secret: access_secret.into(),
        }
    }
}

cfg_if! {
    if #[cfg(feature = "egg-mode")] {
        extern crate egg_mode;

        impl<'a> From<Token<'a>> for egg_mode::Token<'a> {
            fn from(t: Token<'a>) -> Self {
                egg_mode::Token::Access {
                    consumer: egg_mode::KeyPair::new(
                        t.consumer_key,
                        t.consumer_secret,
                    ),
                    access: egg_mode::KeyPair::new(
                        t.access_key,
                        t.access_secret,
                    ),
                }
            }
        }
    }
}

cfg_if! {
    if #[cfg(feature = "tweetust")] {
        extern crate tweetust;

        use self::tweetust::conn::{Request, RequestContent};
        use self::tweetust::conn::oauth_authenticator::OAuthAuthorizationScheme;

        impl<'a> tweetust::conn::Authenticator for Token<'a> {
            type Scheme = OAuthAuthorizationScheme;

            fn create_authorization_header(&self, request: &Request)
                -> Option<OAuthAuthorizationScheme>
            {
                let params = match request.content {
                    RequestContent::WwwForm(ref params) => {
                        Some(params.iter().map(|&(ref k, ref v)| (&**k, &**v)))
                    },
                    _ => None,
                };

                Some(OAuthAuthorizationScheme(authorize(
                    self,
                    request.method.as_ref(),
                    &request.url,
                    params,
                )))
            }
        }
    }
}
