use core::fmt;
use std::error;

use bytes::Bytes;
use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{ImageIndex, ImageManifest, MediaType},
};
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
    MissingHeader(String),
    UnknownContentType,
    UnknownArtifactType(String),
    UnknownArchitecture(String),
    AuthenticationRequired,
    Request(reqwest::Error),
    OciError(ErrorResponse),
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

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Error::Request(value)
    }
}

impl From<ErrorResponse> for Error {
    fn from(value: ErrorResponse) -> Self {
        Error::OciError(value)
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

    /// Return the Url for the given URI
    fn build_url(&self, uri: &str) -> Url {
        let mut new_url = self.registry.clone();
        new_url.set_path(uri);
        new_url
    }

    /// Send and log a request
    fn send(&self, request: RequestBuilder) -> reqwest::Result<blocking::Response> {
        let request = request.build()?;
        let method = request.method().to_string();
        let url = request.url().to_string();
        let response = self.client.execute(request)?;
        let status = response.status();
        eprintln!("HTTP: [{method}] {status} {url}");
        Ok(response)
    }

    /// Create and authenticate a new Client with the OCI registry
    /// using the Token authentication as defined by the
    /// [Distribution Registry](https://distribution.github.io/distribution/spec/auth/token/)
    ///
    /// If the registry does not require authentication, self is returned unchanged.
    /// If the registry does require authentication, a new [Client] is returned, consuming self.
    pub fn authenticate(
        self,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, Error> {
        // Try the /v2/ endpoint without authentication to see if we need it.
        let response = self.send(self.client.get(self.build_url("/v2/")))?;
        let status = response.status();
        if status.is_success() {
            // Request was 2xx, not authentication needed
            return Ok(self);
        };

        if status != StatusCode::UNAUTHORIZED {
            return Err(Error::InvalidResponseCode(status.into()));
        };

        // We need authentication, from this point username and password are mandatory
        let (Some(username), Some(password)) = (username, password) else {
            return Err(Error::AuthenticationRequired);
        };

        // Authenticate with the WWW-Authenticate header location
        let www_auth = match response.headers().get(header::WWW_AUTHENTICATE) {
            None => return Err(Error::MissingHeader(header::WWW_AUTHENTICATE.to_string())),
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

        let response: AuthResponse = self.send(request)?.error_for_status()?.json()?;

        let mut headers = header::HeaderMap::new();
        let mut token_header =
            header::HeaderValue::from_str(&format!("Bearer {}", &response.token))
                .expect("valid header");
        token_header.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, token_header);

        let builder = blocking::Client::builder()
            .https_only(true)
            .default_headers(headers);
        Ok(Client {
            client: builder.build().expect("build client"),
            ..self
        })
    }

    /// List all files for the given package
    ///
    /// Includes all versions and files of each version.
    /// Can take a long time for packages with a lot of versions and files.
    pub fn list_package_files(&self, package: &crate::package::Info) -> Result<Vec<String>, Error> {
        let tags = self.list_tags(&package.oci_name())?;
        let mut files: Vec<String> = Vec::new();
        // TODO: Listing tags can be done in parallel
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

    /// Pull an OCI manifest by name and reference
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pulling-manifests
    fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest, Error> {
        let url = self.build_url(&format!("/v2/{name}/manifests/{reference}"));
        let response = self.send(self.client.get(url).header(
            header::ACCEPT,
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        ))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::OciError(response.json::<ErrorResponse>()?));
        };
        match response.headers().get(header::CONTENT_TYPE) {
            Some(value) if value == "application/vnd.oci.image.index.v1+json" => {
                Ok(Manifest::Index(Box::new(response.json::<ImageIndex>()?)))
            }
            Some(value) if value == "application/vnd.oci.image.manifest.v1+json" => Ok(
                Manifest::Manifest(Box::new(response.json::<ImageManifest>()?)),
            ),
            Some(_) => Err(Error::UnknownContentType),
            None => Err(Error::MissingHeader(header::CONTENT_TYPE.to_string())),
        }
    }

    /// Pull a blob
    fn pull_blob(&self, name: &str, digest: &str) -> Result<Bytes, Error> {
        let url = self.build_url(&format!("/v2/{name}/blobs/{digest}"));
        let response = self.send(self.client.get(url))?.error_for_status()?;
        Ok(response.bytes()?)
    }

    /// List all tags by name
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags
    fn list_tags(&self, name: &str) -> Result<TagList, Error> {
        let url = self.build_url(&format!("/v2/{name}/tags/list"));
        let response = self.send(self.client.get(url))?.error_for_status()?;
        let tags = response.json::<TagList>()?;
        Ok(tags)
    }

    pub fn download_package_file(&self, package: &crate::package::Info) -> Result<Bytes, Error> {
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
        self.pull_blob(&package.oci_name(), blob_descriptor.digest())
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
