use anyhow::{anyhow, Context as _, Result};
use futures::{ready, FutureExt};
use http::StatusCode;
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use url::Url;

use crate::pyoci::{AuthResponse, WwwAuth};

/// Authentication layer for the OCI registry
/// This layer will handle [token authentication](https://distribution.github.io/distribution/spec/auth/token/)
/// based on the authentication header of the original request.
#[derive(Debug, Default, Clone)]
pub struct AuthLayer {
    // The Basic token to trade for a Bearer token
    basic: Option<http::HeaderValue>,
    // The Bearer token to use for authentication
    // Will be set after successful authentication
    bearer: Arc<RwLock<Option<http::HeaderValue>>>,
}

impl AuthLayer {
    pub fn new(basic_token: Option<String>) -> Result<Self> {
        let basic_token = match basic_token {
            None => None,
            Some(token) => {
                let mut token = http::HeaderValue::try_from(token)?;
                token.set_sensitive(true);
                Some(token)
            }
        };

        Ok(Self {
            basic: basic_token,
            bearer: Arc::new(RwLock::new(None)),
        })
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, service: S) -> Self::Service {
        AuthService::new(self.basic.clone(), self.bearer.clone(), service)
    }
}

#[derive(Debug, Clone)]
pub struct AuthService<S> {
    basic: Option<http::HeaderValue>,
    bearer: Arc<RwLock<Option<http::HeaderValue>>>,
    service: S,
}

impl<S> AuthService<S> {
    fn new(
        basic: Option<http::HeaderValue>,
        bearer: Arc<RwLock<Option<http::HeaderValue>>>,
        service: S,
    ) -> Self {
        Self {
            bearer,
            basic,
            service,
        }
    }
}

impl<S> Service<reqwest::Request> for AuthService<S>
where
    S: Service<reqwest::Request, Response = reqwest::Response> + Clone + Send + 'static,
    <S as Service<reqwest::Request>>::Future: Send,
    <S as Service<reqwest::Request>>::Error: Into<anyhow::Error>,
{
    type Response = S::Response;
    type Error = anyhow::Error;
    type Future = AuthFuture<S, reqwest::Request>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut request: reqwest::Request) -> Self::Future {
        if let Some(bearer) = self.bearer.read().expect("Failed to get read lock").clone() {
            // If we have a bearer token, add it to the request
            request
                .headers_mut()
                .insert(http::header::AUTHORIZATION, bearer);
        }
        AuthFuture::new(
            request.try_clone(),
            self.clone(),
            self.service.call(request),
        )
    }
}

/// The Future returned by AuthService
/// Implements the actual authentication logic
#[pin_project]
pub struct AuthFuture<S, Req>
where
    S: Service<Req>,
{
    // Clone of the original request to retry after authentication
    request: Option<Req>,
    // Clone of the original service, used to do the authentication request and retry
    // the original request
    auth: AuthService<S>,
    // State of this Future
    #[pin]
    state: AuthState<S::Future>,
}

/// State machine for AuthFuture
#[pin_project(project = AuthStateProj)]
enum AuthState<F> {
    // Polling the original request or the retry after authentication
    Called {
        #[pin]
        future: F,
    },
    // Polling the authentication request
    Authenticating {
        #[pin]
        future: Pin<Box<dyn Future<Output = Result<http::HeaderValue, AuthError>> + Send>>,
    },
}

impl<S, Req> AuthFuture<S, Req>
where
    S: Service<Req>,
{
    fn new(request: Option<Req>, inner: AuthService<S>, future: S::Future) -> Self {
        Self {
            request,
            auth: inner,
            state: AuthState::Called { future },
        }
    }
}

impl<S> Future for AuthFuture<S, reqwest::Request>
where
    // Service being called that we might need to authenticate for
    S: Service<reqwest::Request, Response = reqwest::Response> + Clone + Send + 'static,
    <S as Service<reqwest::Request>>::Future: Send,
    <S as Service<reqwest::Request>>::Error: Into<anyhow::Error>,
{
    type Output = anyhow::Result<reqwest::Response>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                // Polling original request
                AuthStateProj::Called { future } => {
                    let response = ready!(future.poll(cx)).map_err(Into::into)?;

                    if response.status() != StatusCode::UNAUTHORIZED {
                        return Poll::Ready(Ok(response));
                    }
                    tracing::debug!("Received 401 response, authenticating");
                    if this.request.is_none() {
                        // No clone of the original request, can't retry after authentication
                        tracing::debug!("No request to retry, skipping authentication");
                        return Poll::Ready(Ok(response));
                    }
                    // Take the basic token, we are only expected to trade it once
                    let Some(basic_token) = this.auth.basic.take() else {
                        // No basic token to trade for a bearer token
                        tracing::debug!("No basic token, skipping authentication");
                        return Poll::Ready(Ok(response));
                    };

                    let www_auth = match response.headers().get("WWW-Authenticate") {
                        None => {
                            tracing::debug!("No WWW-Authenticate header, skipping authentication");
                            return Poll::Ready(Ok(response));
                        }
                        Some(value) => match WwwAuth::parse(value) {
                            Ok(value) => value,
                            Err(err) => {
                                tracing::error!("Failed to parse WWW-Authenticate header: {}", err);
                                return Poll::Ready(Ok(response));
                            }
                        },
                    };
                    let srv = this.auth.clone();
                    this.state.set(AuthState::Authenticating {
                        // No idea how to type this Future, lets just Pin<Box> it
                        future: authenticate(basic_token, www_auth, srv).boxed(),
                    });
                }
                // Polling authentication request
                AuthStateProj::Authenticating { future } => match ready!(future.poll(cx)) {
                    Ok(bearer_token) => {
                        // Take the original request, this prevents infinitely retrying if the
                        // server keeps returning 401
                        let mut request = this
                            .request
                            .take()
                            .ok_or_else(|| anyhow!("Tried to retry twice after authentication"))?;
                        request
                            .headers_mut()
                            .insert(http::header::AUTHORIZATION, bearer_token.clone());
                        this.auth
                            .bearer
                            .write()
                            .map_err(|_| {
                                anyhow!("Another thread panicked while writing bearer token")
                            })?
                            .replace(bearer_token);
                        // Retry the original request with the new bearer token
                        this.state.set(AuthState::Called {
                            future: this.auth.service.call(request),
                        });
                    }
                    Err(err) => match err {
                        // Error during authentication, return the authentication response
                        AuthError::AuthResponse(auth_response) => {
                            return Poll::Ready(Ok(auth_response))
                        }
                        // Other error, return it
                        AuthError::Error(err) => return Poll::Ready(Err(err)),
                    },
                },
            };
        }
    }
}

enum AuthError {
    AuthResponse(reqwest::Response),
    Error(anyhow::Error),
}

impl<E> From<E> for AuthError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        AuthError::Error(err.into())
    }
}

// Returns the bearer token if successful.
// Returns the upstream response of not.
#[cfg_attr(target_arch = "wasm32", worker::send)]
async fn authenticate<S>(
    basic_token: http::HeaderValue,
    www_auth: WwwAuth,
    mut service: S,
) -> Result<http::HeaderValue, AuthError>
where
    S: Service<reqwest::Request, Response = reqwest::Response>,
    <S as Service<reqwest::Request>>::Future: Send,
    <S as Service<reqwest::Request>>::Error: Into<anyhow::Error>,
{
    let mut auth_url = Url::parse(&www_auth.realm).context("Failed to parse realm URL")?;
    auth_url
        .query_pairs_mut()
        .append_pair("grant_type", "password")
        .append_pair("service", &www_auth.service);
    let mut auth_request = reqwest::Request::new(http::Method::GET, auth_url);
    auth_request
        .headers_mut()
        .append("Authorization", basic_token);
    let response = service.call(auth_request).await?;
    if response.status() != StatusCode::OK {
        return Err(AuthError::AuthResponse(response));
    }
    let auth = response
        .json::<AuthResponse>()
        .await
        .context("Failed to parse authentication response")?;
    let mut token = http::HeaderValue::try_from(format!("Bearer {}", auth.token))
        .context("Failed to create bearer token header")?;
    token.set_sensitive(true);
    Ok(token)
}
