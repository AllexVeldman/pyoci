use core::fmt;
use std::error;

use regex::Regex;
use reqwest::{
    blocking::{self, RequestBuilder},
    header::{self, HeaderValue},
    StatusCode,
};
use serde::Deserialize;
use url::{ParseError, Url};

#[derive(Debug)]
pub enum Error {
    InvalidUrl(ParseError),
    InvalidResponseCode(u16),
    MissingHeader,
    AuthenticationRequired,
    Request(String),
    Response(String),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl error::Error for Error {}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Error::Other(value.to_string())
    }
}

impl From<ParseError> for Error {
    fn from(value: ParseError) -> Self {
        Error::InvalidUrl(value)
    }
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
}

/// Client to communicate with the OCI v2 registry
pub struct Client {
    /// OCI Registry to connect with
    registry: Url,
    /// client used to send HTTP requests with
    client: blocking::Client,
}



impl Client {
    /// Create a new Client
    ///
    /// returns an error if `registry` can't be parsed as an URL
    pub fn new(registry: &str) -> Result<Self, Error> {
        let url = match Url::parse(registry) {
            Ok(value) => value,
            Err(err) => match err {
                ParseError::RelativeUrlWithoutBase => Url::parse(&format!("https://{registry}"))?,
                err => return Err(Error::InvalidUrl(err)),
            },
        };
        let builder = blocking::Client::builder();

        Ok(Client {
            client: builder.build().expect("build client"),
            registry: url,
        })
    }

    /// Create and authenticate a new Client with the OCI registry
    /// using the Token authentication as defined by the
    /// [Distribution Registry](https://distribution.github.io/distribution/spec/auth/token/)
    ///
    /// If the registry does not require authentication, self is returned unchanged.
    /// If the registry does require authentication, a new [Client] is returned, consuming self.
    pub fn authenticate(self, username: Option<&str>, password: Option<&str>) -> Result<Self, Error> {
        // Try the /v2/ endpoint without authentication to see if we need it.
        let response = match self.send(self.client.get(self.build_url("/v2/"))) {
            Err(err) => return Err(Error::Request(err.to_string())),
            Ok(value) => value,
        };
        let status = response.status();
        if status.is_success() {
            // Request was 2xx, not authentication needed
            return Ok(self);
        };

        if status != StatusCode::UNAUTHORIZED {
            return Err(Error::InvalidResponseCode(status.into()));
        };

        // We need authentication, from this point username and password are mandatory
        let (username, password) = match (username, password) {
            (Some(username), Some(password)) => {(username, password)},
            _ => { return Err(Error::AuthenticationRequired) }
        };

        // Authenticate with the WWW-Authenticate header location
        let www_auth = match response.headers().get(header::WWW_AUTHENTICATE) {
            None => return Err(Error::MissingHeader),
            Some(value) => WwwAuth::parse(value)?,
        };

        let request = self
            .client
            .get(www_auth.realm)
            .query(&[
                ("grant_type", "password"),
                ("service", &www_auth.service),
                ("client_id", username),
            ])
            .basic_auth(username, Some(password));

        let response = match self.send(request) {
            Err(err) => return Err(Error::Request(err.to_string())),
            Ok(value) => value,
        };

        let status = response.status();
        if !status.is_success() {
            return Err(Error::InvalidResponseCode(status.into()));
        }

        let response: AuthResponse = match response.json() {
            Err(err) => return Err(Error::Response(err.to_string())),
            Ok(value) => value,
        };

        let mut headers = header::HeaderMap::new();
        let mut token_header =
            header::HeaderValue::from_str(&format!("Bearer {}", &response.token))
                .expect("valid header");
        token_header.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, token_header);

        let builder = blocking::Client::builder()
            .https_only(true)
            .default_headers(headers);
        println!("Authenticated to {}", self.registry);
        Ok(Client {
            client: builder.build().expect("build client"),
            ..self
        })
    }

    /// Return the Url for the given URI
    fn build_url(&self, uri: &str) -> Url {
        let mut new_url = self.registry.clone();
        new_url.set_path(uri);
        new_url
    }

    /// Send and log a request
    fn send(&self, request: RequestBuilder) -> reqwest::Result<blocking::Response>{  
        let request = request.build()?;
        let method = request.method().to_string();
        let url = request.url().to_string();
        let response = self.client.execute(request)?;
        let status = response.status();
        println!("HTTP: [{method}] {status} {url}");
        Ok(response)
    }
}

/// WWW-Authenticate header
/// ref: <https://datatracker.ietf.org/doc/html/rfc6750#section-3>
struct WwwAuth {
    realm: String,
    service: String,
    scope: String,
}

impl WwwAuth {
    fn parse(value: &HeaderValue) -> Result<Self, Error> {
        let value = match value.to_str() {
            Err(_) => return Err("not visible ASCII".into()),
            Ok(value) => value,
        };
        let value = match value.strip_prefix("Bearer ") {
            None => return Err("not bearer".into()),
            Some(value) => value,
        };
        let realm = match Regex::new(r#"realm="(?P<realm>[^"\s]*)"#)
            .expect("valid regex")
            .captures(value)
        {
            Some(value) => value
                .name("realm")
                .expect("realm to be part of match")
                .as_str()
                .to_string(),
            None => return Err("realm missing".into()),
        };
        let service = match Regex::new(r#"service="(?P<service>[^"\s]*)"#)
            .expect("valid regex")
            .captures(value)
        {
            Some(value) => value
                .name("service")
                .expect("service to be part of match")
                .as_str()
                .to_string(),
            None => return Err("service missing".into()),
        };
        let scope = match Regex::new(r#"scope="(?P<scope>[^"]*)"#)
            .expect("valid regex")
            .captures(value)
        {
            Some(value) => value
                .name("scope")
                .expect("scope to be part of match")
                .as_str()
                .to_string(),
            None => return Err("scope missing".into()),
        };
        Ok(WwwAuth {
            realm,
            service,
            scope,
        })
    }
}
