use core::fmt;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use reqwest::Response;
use std::error;
use url::{ParseError, Url};

use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{Descriptor, ImageIndex, ImageManifest, MediaType},
};
use regex::Regex;
use serde::Deserialize;

use crate::package;
use crate::transport::HttpTransport;

#[derive(Debug)]
pub enum Error {
    Package(package::ParseError),
    InvalidUrl(ParseError),
    InvalidResponseCode(u16),
    MissingHeader(String),
    UnknownContentType,
    UnknownArtifactType(String),
    UnknownArchitecture(String),
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

impl From<package::ParseError> for Error {
    fn from(value: package::ParseError) -> Self {
        Error::Package(value)
    }
}

impl From<ErrorResponse> for Error {
    fn from(value: ErrorResponse) -> Self {
        Error::OciErrorResponse(value)
    }
}

/// Return type for ``pull_manifest``
/// as the same endpoint can return both a manifest and a manifest index
#[derive(Debug)]
pub enum Manifest {
    Index(Box<ImageIndex>),
    Manifest(Box<ImageManifest>),
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

/// WWW-Authenticate header
/// ref: <https://datatracker.ietf.org/doc/html/rfc6750#section-3>
pub struct WwwAuth {
    pub realm: String,
    pub service: String,
    // scope: String,
}

impl WwwAuth {
    pub fn parse(value: &str) -> Result<Self, Error> {
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

/// Client to communicate with the OCI v2 registry
pub struct PyOci {
    registry: Url,
    transport: HttpTransport,
}

impl PyOci {
    /// Create a new Client
    pub fn new(registry: Url, auth: Option<String>) -> Self {
        PyOci {
            registry,
            transport: HttpTransport::new(auth),
        }
    }

    fn build_url(&self, uri: &str) -> Url {
        let mut new_url = self.registry.clone();
        new_url.set_path(uri);
        new_url
    }

    /// List all files for the given package
    ///
    /// Includes all versions and files of each version.
    /// Can take a long time for packages with a lot of versions and files.
    pub async fn list_package_files(
        &self,
        package: &package::Info,
    ) -> Result<Vec<package::Info>, Error> {
        let tags = self.list_tags(&package.oci_name()).await?;
        let mut files: Vec<package::Info> = Vec::new();
        let futures = FuturesUnordered::new();

        for tag in tags.tags() {
            futures.push(self.package_info_for_ref(package, tags.name(), tag));
        }
        for result in futures
            .collect::<Vec<Result<Vec<package::Info>, Error>>>()
            .await
        {
            match result {
                Ok(mut value) => files.append(&mut value),
                Err(err) => return Err(err),
            }
        }
        Ok(files)
    }

    async fn package_info_for_ref(
        &self,
        package: &package::Info,
        name: &str,
        reference: &str,
    ) -> Result<Vec<package::Info>, Error> {
        let manifest = self.pull_manifest(name, reference).await?;
        let Manifest::Index(index) = manifest else {
            return Err(Error::Other("Expected Index, got Manifest".to_string()));
        };
        let artifact_type = index.artifact_type();
        match artifact_type {
            // Artifact type is as expected, do nothing
            Some(MediaType::Other(value)) if value == "application/pyoci.package.v1" => {}
            // Artifact type has unexpected value, err
            Some(value) => return Err(Error::UnknownArtifactType(value.to_string())),
            // Artifact type is not set, err
            None => return Err(Error::UnknownArtifactType(String::new())),
        };
        let mut files: Vec<package::Info> = Vec::new();
        for manifest in index.manifests() {
            match manifest.platform().as_ref().unwrap().architecture() {
                oci_spec::image::Arch::Other(arch) => {
                    let mut file = package.clone();
                    file.file = package
                        .file
                        .clone()
                        .with_version(reference)
                        .with_architecture(arch)
                        .unwrap();
                    files.push(file);
                }
                arch => return Err(Error::UnknownArchitecture(arch.to_string())),
            };
        }
        Ok(files)
    }

    pub async fn download_package_file(
        &self,
        package: &crate::package::Info,
    ) -> Result<Response, Error> {
        if !package.file.is_valid() {
            return Err(Error::NotAFile(package.file.to_string()));
        };
        // Pull index
        let index = match self
            .pull_manifest(&package.oci_name(), &package.file.version)
            .await?
        {
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
        let manifest = match self
            .pull_manifest(&package.oci_name(), manifest_descriptor.digest())
            .await?
        {
            Manifest::Index(_) => {
                return Err(Error::Other("Expected Manifest, got Index".to_string()))
            }
            Manifest::Manifest(manifest) => manifest,
        };
        // pull blob in first layer of manifest
        let [blob_descriptor] = &manifest.layers()[..] else {
            return Err(Error::Other("Unsupported number of layers".to_string()));
        };
        self.pull_blob(package.oci_name(), blob_descriptor.to_owned())
            .await
    }

    pub async fn publish_package_file(
        &self,
        package: &crate::package::Info,
        file: &str,
    ) -> Result<(), Error> {
        todo!()
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
        // Ok(())
    }
}

impl PyOci {
    /// Pull a blob from the registry
    ///
    /// This returns the raw response so the caller can handle the blob as needed
    async fn pull_blob(&self, name: String, descriptor: Descriptor) -> Result<Response, Error> {
        let digest = descriptor.digest();
        let url = self.build_url(&format!("/v2/{name}/blobs/{digest}"));
        let request = self.transport.get(url);
        let response = self.transport.send(request).await.expect("valid response");

        let status: u16 = response.status().into();
        if !status == 200 {
            return Err(Error::InvalidResponseCode(status));
        };
        Ok(response)
    }
    async fn list_tags(&self, name: &str) -> Result<TagList, Error> {
        let url = self.build_url(&format!("/v2/{name}/tags/list"));
        let request = self.transport.get(url);
        let response = self.transport.send(request).await.expect("valid response");
        let status: u16 = response.status().into();
        if !(200..=299).contains(&status) {
            return Err(Error::OciErrorResponse(
                response
                    .json::<ErrorResponse>()
                    .await
                    .expect("valid Error json"),
            ));
        };
        let tags = response
            .json::<TagList>()
            .await
            .expect("valid TagList json");
        Ok(tags)
    }
    async fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest, Error> {
        let url = self.build_url(&format!("/v2/{name}/manifests/{reference}"));
        let request = self.transport.get(url).header(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        );
        let response = self.transport.send(request).await.expect("valid response");
        let status: u16 = response.status().into();
        if !(200..299).contains(&status) {
            return Err(Error::OciErrorResponse(
                response.json::<ErrorResponse>().await.expect("valid json"),
            ));
        };
        match response.headers().get("Content-Type") {
            Some(value) if value == "application/vnd.oci.image.index.v1+json" => {
                Ok(Manifest::Index(Box::new(
                    response
                        .json::<ImageIndex>()
                        .await
                        .expect("valid Index json"),
                )))
            }
            Some(value) if value == "application/vnd.oci.image.manifest.v1+json" => {
                Ok(Manifest::Manifest(Box::new(
                    response
                        .json::<ImageManifest>()
                        .await
                        .expect("valid Manifest json"),
                )))
            }
            Some(_) => Err(Error::UnknownContentType),
            None => Err(Error::MissingHeader("Content-Type".to_string())),
        }
    }
}

// fn parse_auth(value: &str) -> (Option<String>, Option<String>) {
//     tracing::debug!("Parsing auth header: {:?}", value);
//     let Some(value) = value.strip_prefix("Basic ") else {
//         return (None, None);
//     };
//     match BASE64_STANDARD.decode(value.as_bytes()) {
//         Ok(decoded) => {
//             let decoded = String::from_utf8(decoded).expect("valid utf8");
//             match decoded.splitn(2, ':').collect::<Vec<&str>>()[..] {
//                 [username, password] => (Some(username.to_string()), Some(password.to_string())),
//                 _ => (None, None),
//             }
//         }
//         Err(err) => {
//             tracing::warn!("Failed to decode auth header: {:?}", err);
//             (None, None)
//         }
//     }
// }
