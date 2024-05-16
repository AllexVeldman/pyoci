use core::fmt;
use std::{error, io::Read};

use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{Descriptor, ImageIndex, ImageManifest, MediaType},
};
use regex::Regex;
use serde::Deserialize;

use url::ParseError;

use crate::package;

#[derive(Debug)]
pub enum Error {
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

/// Generic trait for OCI transport
///
/// Allows swapping out the transport implementation on Client
#[allow(async_fn_in_trait)]
pub trait OciTransport {
    fn with_auth(self, username: Option<String>, password: Option<String>) -> Self;
    async fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest, Error>;
    async fn pull_blob(&self, name: String, descriptor: Descriptor) -> Result<impl Read, Error>;
    async fn list_tags(&self, name: &str) -> Result<TagList, Error>;
}

/// Client to communicate with the OCI v2 registry
pub struct Client<T: OciTransport> {
    /// Transport to use
    transport: T,
}

impl<T: OciTransport> Client<T> {
    /// Create a new Client
    ///
    /// returns an error if `registry` can't be parsed as an URL
    pub fn new(transport: T) -> Self {
        Client { transport }
    }

    /// List all files for the given package
    ///
    /// Includes all versions and files of each version.
    /// Can take a long time for packages with a lot of versions and files.
    pub async fn list_package_files(
        &self,
        package: &package::Info,
    ) -> Result<Vec<package::Info>, Error> {
        let tags = self.transport.list_tags(&package.oci_name()).await?;
        let mut files: Vec<package::Info> = Vec::new();
        for tag in tags.tags() {
            let manifest = self.transport.pull_manifest(tags.name(), tag).await?;
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
                                let mut file = package.clone();
                                file.file = package
                                    .file
                                    .clone()
                                    .with_version(tag)
                                    .with_architecture(arch)
                                    .unwrap();
                                files.push(file);
                            }
                            arch => return Err(Error::UnknownArchitecture(arch.to_string())),
                        };
                    }
                }
            };
        }
        Ok(files)
    }

    pub async fn download_package_file<'a>(
        &'a self,
        package: &crate::package::Info,
    ) -> Result<impl Read + 'a, Error> {
        if !package.file.is_valid() {
            return Err(Error::NotAFile(package.file.to_string()));
        };
        // Pull index
        let index = match self
            .transport
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
            .transport
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
        self.transport
            .pull_blob(package.oci_name(), blob_descriptor.to_owned())
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
