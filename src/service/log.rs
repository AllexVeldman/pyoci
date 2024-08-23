use futures::ready;
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use time::OffsetDateTime;
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
    S: Service<reqwest::Request, Response = reqwest::Response>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = LogFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: reqwest::Request) -> Self::Future {
        LogFuture {
            method: request.method().to_string(),
            url: request.url().to_string(),
            inner_fut: self.inner.call(request),
            request_type: self.request_type,
            start: OffsetDateTime::now_utc(),
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
    start: OffsetDateTime,
}

impl<F, Error> Future for LogFuture<F>
where
    F: Future<Output = Result<reqwest::Response, Error>>,
{
    type Output = Result<reqwest::Response, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let result = ready!(this.inner_fut.poll(cx));
        if let Ok(response) = &result {
            let status: u16 = response.status().into();
            tracing::info!(
                elapsed_ms = (OffsetDateTime::now_utc() - *this.start).whole_milliseconds(),
                method = this.method,
                status,
                url = this.url,
                "type" = this.request_type,
            );
        }
        Poll::Ready(result)
    }
}
