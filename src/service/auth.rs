use futures::ready;
use http::StatusCode;
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use url::Url;

use crate::pyoci::{AuthResponse, WwwAuth};

#[derive(Debug, Default, Clone)]
pub struct AuthLayer {
    // The Basic token to trade for a Bearer token
    basic: Option<http::HeaderValue>,
    // The Bearer token to use for authentication
    // Will be updated after successful authentication
    bearer: Arc<RwLock<Option<http::HeaderValue>>>,
}

impl AuthLayer {
    pub fn new(basic_token: Option<String>) -> Self {
        let basic_token = basic_token.map(|token| {
            let mut token =
                http::HeaderValue::try_from(token).expect("Failed to create basic token");
            token.set_sensitive(true);
            token
        });

        Self {
            basic: basic_token,
            bearer: Arc::new(RwLock::new(None)),
        }
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
    pub fn new(
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
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = AuthFuture<S>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
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

#[pin_project(project = AuthStateProj)]
enum AuthState<F, A> {
    Called {
        #[pin]
        future: F,
    },
    Authenticating {
        #[pin]
        future: A,
    },
}

#[pin_project]
pub struct AuthFuture<S>
where
    S: Service<reqwest::Request, Response = reqwest::Response> + Clone + 'static,
    <S as Service<reqwest::Request>>::Future: Send,
{
    // Clone of the original request to retry after authentication
    request: Option<reqwest::Request>,
    // inner service to call after authenticating
    auth: AuthService<S>,
    // State of this Future
    #[pin]
    state: AuthState<
        S::Future,
        Pin<Box<dyn Future<Output = Result<http::HeaderValue, reqwest::Response>> + Send>>,
    >,
}

impl<S> AuthFuture<S>
where
    S: Service<reqwest::Request, Response = reqwest::Response> + Clone + 'static,
    <S as Service<reqwest::Request>>::Future: Send,
{
    pub fn new(
        request: Option<reqwest::Request>,
        inner: AuthService<S>,
        future: S::Future,
    ) -> Self {
        Self {
            request,
            auth: inner,
            state: AuthState::Called { future },
        }
    }
}

impl<S> Future for AuthFuture<S>
where
    // Service being called that we might need to authenticate for
    S: Service<reqwest::Request, Response = reqwest::Response> + Clone + Send + 'static,
    <S as Service<reqwest::Request>>::Future: Send,
{
    type Output = Result<reqwest::Response, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                // Polling original request
                AuthStateProj::Called { future } => {
                    let response = ready!(future.poll(cx))?;

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
                        Some(value) => match WwwAuth::parse(
                            value
                                .to_str()
                                .expect("Header contains non-ASCII characters"),
                        ) {
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
                        future: Box::pin(async { authenticate(basic_token, www_auth, srv).await }),
                    });
                }
                // Polling authentication request
                AuthStateProj::Authenticating { future } => match ready!(future.poll(cx)) {
                    Ok(bearer_token) => {
                        // Take the original request, this prevent infinitely retrying if the
                        // server keeps returning 401
                        let mut request = this.request.take().expect("Failed to take request");
                        request
                            .headers_mut()
                            .insert(http::header::AUTHORIZATION, bearer_token.clone());
                        this.auth
                            .bearer
                            .write()
                            .expect("Failed to get write lock")
                            .replace(bearer_token);
                        // Retry the original request with the new bearer token
                        this.state.set(AuthState::Called {
                            future: this.auth.service.call(request),
                        });
                    }
                    Err(response) => return Poll::Ready(Ok(response)),
                },
            };
        }
    }
}

// Returns the bearer token if successful.
// Returns the upstream response of not.
#[cfg_attr(target_arch = "wasm32", worker::send)]
async fn authenticate<S>(
    basic_token: http::HeaderValue,
    www_auth: WwwAuth,
    mut service: AuthService<S>,
    // TODO: Figure out how to do error propagation
) -> Result<http::HeaderValue, reqwest::Response>
where
    S: Service<reqwest::Request, Response = reqwest::Response> + Clone + Send + 'static,
    <S as Service<reqwest::Request>>::Future: Send,
{
    let mut auth_url = Url::parse(&www_auth.realm).expect("Failed to parse realm URL");
    auth_url
        .query_pairs_mut()
        .append_pair("grant_type", "password")
        .append_pair("service", &www_auth.service);
    let mut auth_request = reqwest::Request::new(http::Method::GET, auth_url);
    auth_request
        .headers_mut()
        .append("Authorization", basic_token);
    let Ok(response) = service.call(auth_request).await else {
        todo!("Handle error");
    };
    if response.status() != StatusCode::OK {
        return Err(response);
    }
    let auth = response
        .json::<AuthResponse>()
        .await
        .expect("Failed to parse auth response");
    let mut token = http::HeaderValue::try_from(format!("Bearer {}", auth.token))
        .expect("Failed to create bearer token header");
    token.set_sensitive(true);
    Ok(token)
}
