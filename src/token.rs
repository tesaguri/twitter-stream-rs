use std::borrow::Borrow;

use oauth_credentials::Credentials;

/// An OAuth token used to authenticate to Twitter.
#[derive(Copy, Clone, Debug)]
pub struct Token<C = String, T = String> {
    /// Client credentials (aka consumer key and consumer secret, or API key and API secret key)
    pub client: Credentials<C>,
    /// Token credentials (aka access token and access token secret)
    pub token: Credentials<T>,
}

impl<C: Borrow<str>, T: Borrow<str>> Token<C, T> {
    /// Creates a new `Token` with the specified client credentials and token credentials.
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

    /// Creates a new `Token` from `Credentials`.
    pub fn from_credentials(client: Credentials<C>, token: Credentials<T>) -> Self {
        Self { client, token }
    }

    /// Borrow token strings from `self` and make a new `Token` with them.
    pub fn as_ref(&self) -> Token<&str, &str> {
        Token::from_credentials(self.client.as_ref(), self.token.as_ref())
    }
}
