#[cfg(feature = "egg-mode")]
extern crate egg_mode;
#[cfg(feature = "tweetust")]
extern crate tweetust;

use std::borrow::Cow;

/// An OAuth token used to log into Twitter.
#[cfg_attr(feature = "tweetust", doc = "

This implements `tweetust::conn::Authenticator` so you can pass it to `tweetust::TwitterClient`
as if it were `tweetust::OAuthAuthenticator`"
)]
#[cfg_attr(feature = "use-serde", derive(Deserialize, Serialize))]
#[derive(Clone, Debug)]
pub struct Token<'a> {
    pub consumer_key: Cow<'a, str>,
    pub consumer_secret: Cow<'a, str>,
    pub access_key: Cow<'a, str>,
    pub access_secret: Cow<'a, str>,
}

impl<'a> Token<'a> {
    pub fn new<CK, CS, AK, AS>(consumer_key: CK, consumer_secret: CS, access_key: AK, access_secret: AS) -> Self
        where CK: Into<Cow<'a, str>>, CS: Into<Cow<'a, str>>, AK: Into<Cow<'a, str>>, AS: Into<Cow<'a, str>>
    {
        Token {
            consumer_key: consumer_key.into(),
            consumer_secret: consumer_secret.into(),
            access_key: access_key.into(),
            access_secret: access_secret.into(),
        }
    }
}

#[cfg(feature = "egg-mode")]
impl<'a> From<Token<'a>> for egg_mode::Token<'a> {
    fn from(t: Token<'a>) -> Self {
        egg_mode::Token::Access {
            consumer: egg_mode::KeyPair::new(t.consumer_key, t.consumer_secret),
            access: egg_mode::KeyPair::new(t.access_key, t.access_secret),
        }
    }
}

#[cfg(feature = "tweetust")]
impl<'a> tweetust::conn::Authenticator for Token<'a> {
    type Scheme = tweetust::conn::oauth_authenticator::OAuthAuthorizationScheme;

    fn create_authorization_header(&self, request: &tweetust::conn::Request)
        -> Option<tweetust::conn::oauth_authenticator::OAuthAuthorizationScheme>
    {
        let params = if let tweetust::conn::RequestContent::WwwForm(ref params) = request.content {
            Some(params.as_ref().iter().map(|&(ref k, ref v)| (k.as_ref(), v.as_ref())))
        } else {
            None
        };

        Some(tweetust::conn::oauth_authenticator::OAuthAuthorizationScheme(
            authorize(self, request.method.as_ref(), &request.url, params)
        ))
    }
}
