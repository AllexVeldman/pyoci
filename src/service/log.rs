use futures::ready;
use pin_project::pin_project;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Debug, Default, Clone)]
pub struct RequestLogLayer {
    request_type: &'static str,
}

impl RequestLogLayer {
    pub fn new(request_type: &'static str) -> Self {
        Self { request_type }
    }
}

impl<S> Layer<S> for RequestLogLayer {
    type Service = RequestLog<S>;

    fn layer(&self, service: S) -> Self::Service {
        RequestLog::new(self.request_type, service)
    }
}

#[derive(Debug, Clone)]
pub struct RequestLog<S> {
    request_type: &'static str,
    inner: S,
}

impl<S> RequestLog<S> {
    pub fn new(request_type: &'static str, service: S) -> Self {
        Self {
            request_type,
            inner: service,
        }
    }
}

impl<S> Service<reqwest::Request> for RequestLog<S>
where
    S: Service<reqwest::Request, Response = reqwest::Response, Error = reqwest::Error>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = LogFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: reqwest::Request) -> Self::Future {
        tracing::debug!("{:?}", request);
        LogFuture {
            method: request.method().to_string(),
            url: request.url().to_string(),
            inner_fut: self.inner.call(request),
            request_type: self.request_type,
        }
    }
}

#[pin_project]
pub struct LogFuture<F> {
    #[pin]
    inner_fut: F,
    method: String,
    url: String,
    request_type: &'static str,
}

impl<F> Future for LogFuture<F>
where
    F: Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    type Output = Result<reqwest::Response, reqwest::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let result = ready!(this.inner_fut.poll(cx));
        match &result {
            Ok(response) => {
                tracing::debug!("{:?}", response);
                let status: u16 = response.status().into();
                tracing::info!(
                    method = this.method,
                    "type" = this.request_type,
                    status,
                    url = this.url,
                );
            }
            Err(error) => {
                if let Some(source) = error.source() {
                    tracing::error!(
                        method = this.method,
                        "type" = this.request_type,
                        url = this.url,
                        error = format!("{source}")
                    );
                } else {
                    tracing::debug!(
                        method = this.method,
                        "type" = this.request_type,
                        url = this.url,
                        error = format!("{error:?}")
                    );
                }
            }
        }
        Poll::Ready(result)
    }
}
