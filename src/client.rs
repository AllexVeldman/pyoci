use core::fmt;
use std::{error, io::Read, sync::Arc, sync::Mutex, time::Duration};

use base64::prelude::{Engine as _, BASE64_STANDARD};
use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{Descriptor, ImageIndex, ImageManifest, MediaType},
};
use regex::Regex;
use serde::Deserialize;
use ureq::Middleware;
use url::{ParseError, Url};

#[derive(Debug)]
pub enum Error {
    InvalidUrl(ParseError),
    InvalidResponseCode(u16),
    MissingHeader(String),
    UnknownContentType,
    UnknownArtifactType(String),
    UnknownArchitecture(String),
    Request(Box<ureq::Error>),
    OciErrorResponse(ErrorResponse),
    NotAFile(String),
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

impl From<ureq::Error> for Error {
    fn from(value: ureq::Error) -> Self {
        Error::Request(Box::new(value))
    }
}

impl From<ErrorResponse> for Error {
    fn from(value: ErrorResponse) -> Self {
        Error::OciErrorResponse(value)
    }
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
}

/// Return type for ``pull_manifest``
/// as the same endpoint can return both a manifest and a manifest index
#[derive(Debug)]
enum Manifest {
    Index(Box<ImageIndex>),
    Manifest(Box<ImageManifest>),
}

/// Client to communicate with the OCI v2 registry
pub struct Client {
    /// OCI Registry to connect with
    registry: Url,
    /// client used to send HTTP requests with
    client: ureq::Agent,
}

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
            //  TODO: set token
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
            token.clone().unwrap()
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

impl Client {
    /// Create a new Client
    ///
    /// returns an error if `registry` can't be parsed as an URL
    pub fn new(
        registry: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, Error> {
        let url = match Url::parse(registry) {
            Ok(value) => value,
            Err(err) => match err {
                ParseError::RelativeUrlWithoutBase => Url::parse(&format!("https://{registry}"))?,
                err => return Err(Error::InvalidUrl(err)),
            },
        };

        Ok(Client {
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
            registry: url,
        })
    }

    /// Return the Url for the given URI
    fn build_url(&self, uri: &str) -> String {
        let mut new_url = self.registry.clone();
        new_url.set_path(uri);
        new_url.to_string()
    }

    /// Pull an OCI manifest by name and reference
    /// <https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pulling-manifests>
    fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest, Error> {
        let url = self.build_url(&format!("/v2/{name}/manifests/{reference}"));
        let response = self.client.get(&url).set(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        ).call()?;
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
    fn pull_blob(&self, name: &str, descriptor: &Descriptor) -> Result<impl Read, Error> {
        let digest = descriptor.digest();
        let url = self.build_url(&format!("/v2/{name}/blobs/{digest}"));
        let response = self.client.get(&url).call()?;

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
        let response = self.client.get(&url).call()?;
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

    /// List all files for the given package
    ///
    /// Includes all versions and files of each version.
    /// Can take a long time for packages with a lot of versions and files.
    pub fn list_package_files(&self, package: &crate::package::Info) -> Result<Vec<String>, Error> {
        let tags = self.list_tags(&package.oci_name())?;
        let mut files: Vec<String> = Vec::new();
        for tag in tags.tags() {
            let manifest = self.pull_manifest(tags.name(), tag)?;
            match manifest {
                Manifest::Manifest(_) => {
                    return Err(Error::Other(
                        "Manifest without Index not supported".to_string(),
                    ))
                }
                Manifest::Index(index) => {
                    let artifact_type = index.artifact_type();
                    match artifact_type {
                        // Artifact type is as expected, do nothing
                        Some(MediaType::Other(value))
                            if value == "application/pyoci.package.v1" => {}
                        // Artifact type has unexpected value, err
                        Some(value) => return Err(Error::UnknownArtifactType(value.to_string())),
                        // Artifact type is not set, err
                        None => return Err(Error::UnknownArtifactType(String::new())),
                    };
                    for manifest in index.manifests() {
                        match manifest.platform().as_ref().unwrap().architecture() {
                            oci_spec::image::Arch::Other(arch) => {
                                let file = package
                                    .file
                                    .clone()
                                    .with_version(tag)
                                    .with_architecture(arch)
                                    .unwrap();
                                files.push(format!("{file}"));
                            }
                            arch => return Err(Error::UnknownArchitecture(arch.to_string())),
                        };
                    }
                }
            };
        }
        Ok(files)
    }

    pub fn download_package_file(
        &self,
        package: &crate::package::Info,
    ) -> Result<impl Read, Error> {
        if !package.file.is_valid() {
            return Err(Error::NotAFile(package.file.to_string()));
        };
        // Pull index
        let index = match self.pull_manifest(&package.oci_name(), &package.file.version)? {
            Manifest::Index(index) => index,
            Manifest::Manifest(_) => {
                return Err(Error::Other("Expected Index, got Manifest".to_string()))
            }
        };
        // Check artifact type
        match index.artifact_type() {
            // Artifact type is as expected, do nothing
            Some(MediaType::Other(value)) if value == "application/pyoci.package.v1" => {}
            // Artifact type has unexpected value, err
            Some(value) => return Err(Error::UnknownArtifactType(value.to_string())),
            // Artifact type is not set, err
            None => return Err(Error::UnknownArtifactType(String::new())),
        };
        // Find manifest descriptor for platform
        let mut platform_manifest: Option<&oci_spec::image::Descriptor> = None;
        for manifest in index.manifests() {
            if let Some(platform) = manifest.platform() {
                match platform.architecture() {
                    oci_spec::image::Arch::Other(arch) if *arch == package.file.architecture() => {
                        platform_manifest = Some(manifest);
                        break;
                    }
                    _ => {}
                }
            }
        }
        let manifest_descriptor = platform_manifest.ok_or(Error::Other(
            "Requested architecture not available".to_string(),
        ))?;
        // pull manifest
        let manifest =
            match self.pull_manifest(&package.oci_name(), manifest_descriptor.digest())? {
                Manifest::Index(_) => {
                    return Err(Error::Other("Expected Manifest, got Index".to_string()))
                }
                Manifest::Manifest(manifest) => manifest,
            };
        // pull blob in first layer of manifest
        let [blob_descriptor] = &manifest.layers()[..] else {
            return Err(Error::Other("Unsupported number of layers".to_string()));
        };
        self.pull_blob(&package.oci_name(), blob_descriptor)
    }

    pub fn publish_package_file(
        &self,
        package: &crate::package::Info,
        file: &str,
    ) -> Result<(), Error> {
        // let url = self.build_url(&format!("/v2/{package.oci_name()}/blobs/uploads/"));
        // let response = self.client.post(&url).call()?;
        // let location = response
        //     .header("Location")
        //     .ok_or(Error::MissingHeader("Location".to_string()))?;
        // let file = std::fs::File::open(file)?;
        // let response = self.client.put(location).send(file)?;
        // let status = response.status();
        // if !(200..=299).contains(&status) {
        //     return Err(Error::InvalidResponseCode(status));
        // };
        Ok(())
    }
}

/// WWW-Authenticate header
/// ref: <https://datatracker.ietf.org/doc/html/rfc6750#section-3>
struct WwwAuth {
    realm: String,
    service: String,
    // scope: String,
}

impl WwwAuth {
    fn parse(value: &str) -> Result<Self, Error> {
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
        // let scope = match Regex::new(r#"scope="(?P<scope>[^"]*)"#)
        //     .expect("valid regex")
        //     .captures(value)
        // {
        //     Some(value) => value
        //         .name("scope")
        //         .expect("scope to be part of match")
        //         .as_str()
        //         .to_string(),
        //     None => return Err("scope missing".into()),
        // };
        Ok(WwwAuth {
            realm,
            service,
            // scope,
        })
    }
}
