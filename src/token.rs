use std::borrow::Borrow;

use cfg_if::cfg_if;
use oauth::Credentials;

/// An OAuth token used to log into Twitter.
#[cfg_attr(
    feature = "tweetust",
    doc = "

This implements `tweetust::conn::Authenticator` so you can pass it to
`tweetust::TwitterClient` as if it were `tweetust::OAuthAuthenticator`"
)]
#[derive(Copy, Clone, Debug)]
pub struct Token<C = String, T = String> {
    pub client: Credentials<C>,
    pub token: Credentials<T>,
}

impl<C: Borrow<str>, T: Borrow<str>> Token<C, T> {
    pub fn new(
        client_identifier: C,
        client_secret: C,
        token_identifier: T,
        token_secret: T,
    ) -> Self {
        let client = Credentials::new(client_identifier, client_secret);
        let token = Credentials::new(token_identifier, token_secret);
        Self::from_credentials(client, token)
    }

    pub fn from_credentials(client: Credentials<C>, token: Credentials<T>) -> Self {
        Self { client, token }
    }

    /// Borrow token strings from `self` and make a new `Token` with them.
    pub fn as_ref(&self) -> Token<&str, &str> {
        Token::from_credentials(self.client.as_ref(), self.token.as_ref())
    }
}

cfg_if! {
    if #[cfg(feature = "egg-mode")] {
        use std::borrow::Cow;

        use egg_mode::KeyPair;

        impl<'a, C, A> From<Token<C, A>> for egg_mode::Token
            where C: Into<Cow<'static, str>>, A: Into<Cow<'static, str>>
        {
            fn from(t: Token<C, A>) -> Self {
                egg_mode::Token::Access {
                    consumer: KeyPair::new(t.client.identifier, t.client.secret),
                    access: KeyPair::new(t.token.identifier, t.token.secret),
                }
            }
        }
    }
}

cfg_if! {
    if #[cfg(feature = "tweetust")] {
        extern crate tweetust_pkg as tweetust;

        use oauthcli::{OAuthAuthorizationHeaderBuilder, SignatureMethod};
        use tweetust::conn::{Request, RequestContent};
        use tweetust::conn::oauth_authenticator::OAuthAuthorizationScheme;

        impl<C, A> tweetust::conn::Authenticator for Token<C, A>
            where C: Borrow<str>, A: Borrow<str>
        {
            type Scheme = OAuthAuthorizationScheme;

            fn create_authorization_header(&self, request: &Request<'_>)
                -> Option<OAuthAuthorizationScheme>
            {
                let mut header = OAuthAuthorizationHeaderBuilder::new(
                    request.method.as_ref(),
                    &request.url,
                    self.client.identifier(),
                    self.client.secret(),
                    SignatureMethod::HmacSha1,
                );
                header.token(self.token.identifier(), self.token.secret());

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
