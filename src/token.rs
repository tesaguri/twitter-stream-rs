use std::borrow::Borrow;

/// An OAuth token used to log into Twitter.
#[cfg_attr(feature = "tweetust",
           doc = "

This implements `tweetust::conn::Authenticator` so you can pass it to
`tweetust::TwitterClient` as if it were `tweetust::OAuthAuthenticator`")]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[derive(Clone, Debug)]
pub struct Token<C = String, A = String> {
    pub consumer_key: C,
    pub consumer_secret: C,
    pub access_key: A,
    pub access_secret: A,
}

impl<C: Borrow<str>, A: Borrow<str>> Token<C, A> {
    pub fn new(consumer_key: C, consumer_secret: C, access_key: A, access_secret: A) -> Self {
        Token {
            consumer_key,
            consumer_secret,
            access_key,
            access_secret,
        }
    }

    /// Borrow token strings from `self` and make a new `Token` with them.
    pub fn borrowed(&self) -> Token<&str, &str> {
        Token::new(
            self.consumer_key.borrow(),
            self.consumer_secret.borrow(),
            self.access_key.borrow(),
            self.access_secret.borrow(),
        )
    }
}

cfg_if! {
    if #[cfg(feature = "egg-mode")] {
        extern crate egg_mode;

        use std::borrow::Cow;

        use self::egg_mode::KeyPair;

        impl<'a, C, A> From<Token<C, A>> for egg_mode::Token
            where C: Into<Cow<'static, str>>, A: Into<Cow<'static, str>>
        {
            fn from(t: Token<C, A>) -> Self {
                egg_mode::Token::Access {
                    consumer: KeyPair::new(t.consumer_key, t.consumer_secret),
                    access: KeyPair::new(t.access_key, t.access_secret),
                }
            }
        }
    }
}

cfg_if! {
    if #[cfg(feature = "use-tweetust")] {
        extern crate oauthcli;
        extern crate tweetust;

        use self::oauthcli::{OAuthAuthorizationHeaderBuilder, SignatureMethod};
        use self::tweetust::conn::{Request, RequestContent};
        use self::tweetust::conn::oauth_authenticator::OAuthAuthorizationScheme;

        impl<C, A> tweetust::conn::Authenticator for Token<C, A>
            where C: Borrow<str>, A: Borrow<str>
        {
            type Scheme = OAuthAuthorizationScheme;

            fn create_authorization_header(&self, request: &Request)
                -> Option<OAuthAuthorizationScheme>
            {
                let mut header = OAuthAuthorizationHeaderBuilder::new(
                    request.method.as_ref(),
                    &request.url,
                    self.consumer_key.borrow(),
                    self.consumer_secret.borrow(),
                    SignatureMethod::HmacSha1,
                );

                if let RequestContent::WwwForm(ref params) = request.content {
                    header.request_parameters(
                        params.iter().map(|&(ref k, ref v)| (&**k, &**v))
                    );
                }

                Some(OAuthAuthorizationScheme(header.finish_for_twitter()))
            }
        }
    }
}
