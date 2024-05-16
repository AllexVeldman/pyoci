use base64::prelude::{Engine as _, BASE64_STANDARD};
use std::{io::Read, sync::Arc, sync::Mutex, time::Duration};
use ureq::Middleware;
use url::ParseError;
use url::Url;

use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{Descriptor, ImageIndex, ImageManifest},
};

use pyoci::client::{AuthResponse, Error, Manifest, OciTransport, WwwAuth};

struct AuthMiddleware {
    username: Option<String>,
    password: Option<String>,
    token: Arc<Mutex<Option<String>>>,
}

impl AuthMiddleware {
    pub fn new(username: Option<String>, password: Option<String>) -> Self {
        AuthMiddleware {
            username,
            password,
            token: Arc::new(Mutex::new(None)),
        }
    }
}

impl Middleware for AuthMiddleware {
    fn handle(
        &self,
        request: ureq::Request,
        next: ureq::MiddlewareNext,
    ) -> Result<ureq::Response, ureq::Error> {
        // add auth header to request if we already have a token
        // If authentication fails it means the token is invalid
        // We're not going to try again with the Basic Auth
        {
            if let Some(token) = &*self.token.lock().unwrap() {
                return next.handle(request.set("Authorization", token));
            };
        }
        // We don't have the token and it's very likely we need to authenticate
        // clone the request before trying so we can try again if we need to
        let request_clone = request.clone();
        // Try the request, if it fails with a 401, authenticate and try again
        let response = next.handle(request)?;
        if response.status() != 401 {
            return Ok(response);
        }
        // Authenticate
        let (Some(username), Some(password)) = (&self.username, &self.password) else {
            // No credentials provided, return the original response
            return Ok(response);
        };
        let www_auth: WwwAuth = match response.header("WWW-Authenticate") {
            None => return Ok(response),
            Some(value) => match WwwAuth::parse(value) {
                Ok(value) => value,
                Err(_) => return Ok(response),
            },
        };

        let basic_auth = BASE64_STANDARD.encode(format!("{username}:{password}").as_bytes());

        let response = ureq::get(&www_auth.realm)
            .set("Authorization", format!("Basic {basic_auth}").as_str())
            .query("grant_type", "password")
            .query("service", &www_auth.service)
            .query("client_id", username)
            .call()?;

        if response.status() != 200 {
            return Ok(response);
        }

        let response: AuthResponse = response.into_json()?;
        {
            let mut token = self.token.lock().unwrap();
            *token = Some(format!("Bearer {}", response.token));
        };

        request_clone.call()
    }
}

// Log all HTTP requests
fn log_middleware(
    request: ureq::Request,
    next: ureq::MiddlewareNext,
) -> Result<ureq::Response, ureq::Error> {
    let method = request.method().to_string();
    let url = request.url().to_string();
    let response = next.handle(request)?;
    let status = response.status();
    tracing::info!("HTTP: [{method}] {status} {url}");
    Ok(response)
}

pub struct SyncTransport {
    client: ureq::Agent,
    registry: Url,
}

impl SyncTransport {
    pub fn new(registry: &str) -> Result<Self, Error> {
        let url = match Url::parse(registry) {
            Ok(value) => value,
            Err(err) => match err {
                ParseError::RelativeUrlWithoutBase => Url::parse(&format!("https://{registry}"))?,
                err => return Err(Error::InvalidUrl(err)),
            },
        };

        Ok(SyncTransport {
            client: ureq::Agent::new(),
            registry: url,
        })
    }
    fn build_url(&self, uri: &str) -> String {
        let mut new_url = self.registry.clone();
        new_url.set_path(uri);
        new_url.to_string()
    }
}

impl OciTransport for SyncTransport {
    /// Add authentication to the client
    fn with_auth(self, username: Option<&str>, password: Option<&str>) -> Self {
        SyncTransport {
            client: ureq::AgentBuilder::new()
                .timeout_read(Duration::from_secs(5))
                .timeout_write(Duration::from_secs(5))
                .https_only(true)
                .middleware(AuthMiddleware::new(
                    username.map(String::from),
                    password.map(String::from),
                ))
                .middleware(log_middleware)
                .build(),
            ..self
        }
    }

    /// Pull an OCI manifest by name and reference
    /// <https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pulling-manifests>
    fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest, Error> {
        let url = self.build_url(&format!("/v2/{name}/manifests/{reference}"));
        let response = self.client.get(&url).set(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        ).call().expect("valid response");
        let status = response.status();
        if !(200 <= status && status <= 299) {
            return Err(Error::OciErrorResponse(
                response.into_json::<ErrorResponse>().expect("valid json"),
            ));
        };
        match response.header("Content-Type") {
            Some(value) if value == "application/vnd.oci.image.index.v1+json" => {
                Ok(Manifest::Index(Box::new(
                    response
                        .into_json::<ImageIndex>()
                        .expect("valid Index json"),
                )))
            }
            Some(value) if value == "application/vnd.oci.image.manifest.v1+json" => {
                Ok(Manifest::Manifest(Box::new(
                    response
                        .into_json::<ImageManifest>()
                        .expect("valid Manifest json"),
                )))
            }
            Some(_) => Err(Error::UnknownContentType),
            None => Err(Error::MissingHeader("Content-Type".to_string())),
        }
    }

    /// Pull a blob
    fn pull_blob(&self, name: String, descriptor: Descriptor) -> Result<impl Read, Error> {
        let digest = descriptor.digest();
        let url = self.build_url(&format!("/v2/{name}/blobs/{digest}"));
        let response = self.client.get(&url).call().expect("valid response");

        let status = response.status();
        if !status == 200 {
            return Err(Error::InvalidResponseCode(status));
        };

        // We have a successful response, download at most size bytes
        let size: u64 = descriptor.size().try_into().expect("valid size");
        let reader = response.into_reader().take(size);

        Ok(reader)
    }

    /// List all tags by name
    /// <https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags>
    fn list_tags(&self, name: &str) -> Result<TagList, Error> {
        let url = self.build_url(&format!("/v2/{name}/tags/list"));
        let response = self.client.get(&url).call().expect("valid response");
        let status = response.status();
        if !(200..=299).contains(&status) {
            return Err(Error::OciErrorResponse(
                response
                    .into_json::<ErrorResponse>()
                    .expect("valid Error json"),
            ));
        };
        let tags = response.into_json::<TagList>().expect("valid TagList json");
        Ok(tags)
    }
}
