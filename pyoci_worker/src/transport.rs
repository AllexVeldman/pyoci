use base64::prelude::{Engine as _, BASE64_STANDARD};
use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{Descriptor, ImageIndex, ImageManifest},
};

use serde::de::DeserializeOwned;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};
use url::Url;
use worker::{CfProperties, Fetch, Headers, Method, Request, RequestInit, Response};

use pyoci::client::{AuthResponse, Error, Manifest, OciTransport, WwwAuth};

// Add to_json method to Response
// as .json() does a check on the Content-Type header
trait Json {
    async fn to_json<T: DeserializeOwned>(&mut self) -> Result<T, Error>;
}

impl Json for Response {
    async fn to_json<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        Ok(serde_json::from_str::<T>(&self.text().await.expect("valid text")).expect("valid json"))
    }
}

#[derive(Default)]
struct Client {
    username: Option<String>,
    password: Option<String>,
    token: Arc<Mutex<Option<String>>>,
}

impl Client {
    async fn send_with_auth(&self, url: &Url, mut request: RequestInit) -> Result<Response, Error> {
        {
            // If we already have a token, add it to the request
            if let Some(token) = &*self.token.lock().unwrap() {
                request
                    .headers
                    .set("Authorization", token)
                    .expect("valid header");
            };
        };
        let response = self.send(url, &request).await.expect("valid response");
        if response.status_code() != 401 {
            return Ok(response);
        }

        // Authenticate
        let www_auth: WwwAuth = match response
            .headers()
            .get("WWW-Authenticate")
            .expect("valid header")
        {
            None => return Ok(response),
            Some(value) => match WwwAuth::parse(&value) {
                Ok(value) => value,
                Err(_) => return Ok(response),
            },
        };
        let (Some(username), Some(password)) = (&self.username, &self.password) else {
            // No credentials provided, return the original response
            return Ok(response);
        };
        let basic_auth = BASE64_STANDARD.encode(format!("{username}:{password}").as_bytes());

        let mut auth_url = Url::parse(&www_auth.realm).expect("valid url");
        auth_url
            .query_pairs_mut()
            .append_pair("grant_type", "password")
            .append_pair("service", &www_auth.service)
            .append_pair("client_id", username);
        let mut auth_request = build_request();
        auth_request
            .headers
            .set("Authorization", format!("Basic {basic_auth}").as_str())
            .expect("valid header");
        let mut auth_response = self
            .send(&auth_url, &auth_request)
            .await
            .expect("valid response");

        if auth_response.status_code() != 200 {
            return Ok(response);
        }

        let auth_response: AuthResponse = auth_response.to_json().await.expect("valid json");
        {
            let mut token = self.token.lock().unwrap();
            let new_token = format!("Bearer {}", auth_response.token);
            *token = Some(new_token.clone());
            request
                .headers
                .set("Authorization", &new_token)
                .expect("valid header");
        };
        self.send(url, &request).await
    }

    #[tracing::instrument(skip(self, url, request_init))]
    async fn send(&self, url: &Url, request_init: &RequestInit) -> Result<Response, Error> {
        let request = Request::new_with_init(url.as_str(), request_init).expect("valid request");
        let response = Fetch::Request(request)
            .send()
            .await
            .expect("valid response");
        tracing::info!(
            "HTTP: [{method}] {status} {url}",
            method = request_init.method.to_string(),
            status = response.status_code(),
            url = url
        );
        Ok(response)
    }
}

// Transport using the javascript fetch API
pub struct JsTransport {
    registry: Url,
    client: Client,
}

impl JsTransport {
    pub fn new(registry: Url) -> Self {
        Self {
            registry,
            client: Client::default(),
        }
    }

    fn build_url(&self, uri: &str) -> Url {
        let mut new_url = self.registry.clone();
        new_url.set_path(uri);
        new_url
    }
}
fn build_request() -> RequestInit {
    let mut headers = Headers::new();
    headers
        .set("User-Agent", "pyoci/0.1-dev (cloudflare worker)")
        .expect("valid header");
    let mut request_init = RequestInit::new();
    request_init
        .with_headers(headers)
        .with_cf_properties(CfProperties {
            apps: Some(false),
            ..CfProperties::default()
        });
    request_init
}

impl OciTransport for JsTransport {
    fn with_auth(self, username: Option<String>, password: Option<String>) -> Self {
        let client = Client {
            username,
            password,
            token: Arc::new(Mutex::new(None)),
        };
        Self { client, ..self }
    }
    async fn pull_blob(&self, name: String, descriptor: Descriptor) -> Result<impl Read, Error> {
        let digest = descriptor.digest();
        let url = self.build_url(&format!("/v2/{name}/blobs/{digest}"));
        let mut request = build_request();
        request.with_method(Method::Get);
        let mut response = self
            .client
            .send_with_auth(&url, request)
            .await
            .expect("valid response");

        let status = response.status_code();
        if !status == 200 {
            return Err(Error::InvalidResponseCode(status));
        };

        let data = response.bytes().await.expect("valid bytes");
        let size: u64 = descriptor.size().try_into().expect("valid size");
        let reader = Cursor::new(data).take(size);

        Ok(reader)
    }
    async fn list_tags(&self, name: &str) -> Result<TagList, Error> {
        let url = self.build_url(&format!("/v2/{name}/tags/list"));
        let mut request = build_request();
        request.with_method(Method::Get);
        let mut response = self
            .client
            .send_with_auth(&url, request)
            .await
            .expect("valid response");
        let status = response.status_code();
        if !(200..=299).contains(&status) {
            return Err(Error::OciErrorResponse(
                response
                    .to_json::<ErrorResponse>()
                    .await
                    .expect("valid Error json"),
            ));
        };
        let tags = response
            .to_json::<TagList>()
            .await
            .expect("valid TagList json");
        Ok(tags)
    }
    async fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest, Error> {
        let url = self.build_url(&format!("/v2/{name}/manifests/{reference}"));
        let mut request = build_request();
        request.with_method(Method::Get);
        request.headers.set(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        ).expect("valid header");
        let mut response = self
            .client
            .send_with_auth(&url, request)
            .await
            .expect("valid response");
        let status = response.status_code();
        if !(200..299).contains(&status) {
            return Err(Error::OciErrorResponse(
                response
                    .to_json::<ErrorResponse>()
                    .await
                    .expect("valid json"),
            ));
        };
        match response
            .headers()
            .get("Content-Type")
            .expect("valid header")
        {
            Some(value) if value == "application/vnd.oci.image.index.v1+json" => {
                Ok(Manifest::Index(Box::new(
                    response
                        .to_json::<ImageIndex>()
                        .await
                        .expect("valid Index json"),
                )))
            }
            Some(value) if value == "application/vnd.oci.image.manifest.v1+json" => {
                Ok(Manifest::Manifest(Box::new(
                    response
                        .to_json::<ImageManifest>()
                        .await
                        .expect("valid Manifest json"),
                )))
            }
            Some(_) => Err(Error::UnknownContentType),
            None => Err(Error::MissingHeader("Content-Type".to_string())),
        }
    }
}
