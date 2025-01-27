use anyhow::{bail, Context, Error, Result};
use axum::response::IntoResponse;
use base16ct::lower::encode_string as hex_encode;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use http::HeaderValue;
use http::StatusCode;
use oci_spec::{
    distribution::TagList,
    image::{
        Arch, Descriptor, DescriptorBuilder, Digest as OciDigest, ImageIndex, ImageIndexBuilder,
        ImageManifest, ImageManifestBuilder, MediaType, Os, Platform, PlatformBuilder,
        Sha256Digest, SCHEMA_VERSION,
    },
};
use reqwest::Response;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::str::FromStr;
use time::format_description::well_known::Rfc3339;
use url::Url;

#[cfg(test)]
use crate::mocks::OffsetDateTime;
#[cfg(not(test))]
use time::OffsetDateTime;

use crate::package::{Package, WithFile, WithoutFile};
use crate::transport::HttpTransport;
use crate::ARTIFACT_TYPE;

/// Build an URL from a format string while sanitizing the parameters
///
/// Note that if the resulting path is an absolute URL, the registry URL is ignored.
/// For more info, see [`Url::join`]
///
/// Returns Err when a parameter fails sanitization
macro_rules! build_url {
    ($pyoci:expr, $uri:literal, $($param:expr),+) => {{
            let uri = format!(
                $uri,
                $(sanitize($param)?,)*
            );
            let mut new_url = $pyoci.registry.clone();
            new_url.set_path("");
            new_url.join(&uri)?
        }}
}

/// Sanitize a string
///
/// Returns an error if the string contains ".."
fn sanitize(value: &str) -> Result<&str> {
    match value {
        value if value.contains("..") => bail!("Invalid value: {}", value),
        value => Ok(value),
    }
}

/// Return type for ``pull_manifest``
/// as the same endpoint can return both a manifest and a manifest index
#[derive(Debug)]
pub enum Manifest {
    Index(Box<ImageIndex>),
    Manifest(Box<ImageManifest>),
}

/// Container for a ImageManifest combined with a Platform
#[derive(Debug)]
struct PlatformManifest {
    manifest: ImageManifest,
    platform: Platform,
}

impl PlatformManifest {
    fn new(manifest: ImageManifest, package: &Package<WithFile>) -> Self {
        let platform = PlatformBuilder::default()
            .architecture(Arch::Other(package.oci_architecture().to_string()))
            .os(Os::Other("any".to_string()))
            .build()
            .expect("valid Platform");
        PlatformManifest { manifest, platform }
    }

    fn descriptor(&self, annotations: HashMap<String, String>) -> Descriptor {
        let (digest, data) = self.digest();
        DescriptorBuilder::default()
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .digest(digest)
            .size(data.len() as u64)
            .platform(self.platform.clone())
            .annotations(annotations)
            .build()
            .expect("Valid PlatformManifest Descriptor")
    }

    fn digest(&self) -> (OciDigest, String) {
        let data = serde_json::to_string(&self.manifest).expect("valid json");
        (digest(&data), data)
    }
}

/// Container for a Blob/Layer data, combined with a Descriptor
struct Blob {
    data: Vec<u8>,
    descriptor: Descriptor,
}

impl Blob {
    fn new(data: Vec<u8>, artifact_type: &str) -> Self {
        let digest = digest(&data);
        let descriptor = DescriptorBuilder::default()
            .media_type(artifact_type)
            .digest(digest)
            .size(data.len() as u64)
            .build()
            .expect("valid Descriptor");
        Blob { data, descriptor }
    }
}

pub fn digest(data: impl AsRef<[u8]>) -> OciDigest {
    let sha = <Sha256 as Digest>::digest(data);
    Sha256Digest::from_str(&hex_encode(&sha))
        .expect("Invalid Digest")
        .into()
}

#[derive(Debug)]
pub struct PyOciError {
    pub status: StatusCode,
    pub message: String,
}
impl std::error::Error for PyOciError {}

impl std::fmt::Display for PyOciError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

impl IntoResponse for PyOciError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

impl From<(StatusCode, &str)> for PyOciError {
    fn from((status, message): (StatusCode, &str)) -> Self {
        PyOciError {
            status,
            message: message.to_string(),
        }
    }
}

impl From<(StatusCode, String)> for PyOciError {
    fn from((status, message): (StatusCode, String)) -> Self {
        PyOciError { status, message }
    }
}

#[derive(Deserialize)]
pub struct AuthResponse {
    #[serde(alias = "access_token")]
    pub token: String,
}

/// Client to communicate with the OCI v2 registry
#[derive(Debug, Clone)]
pub struct PyOci {
    registry: Url,
    transport: HttpTransport,
}

impl PyOci {
    /// Create a new Client
    pub fn new(registry: Url, auth: Option<HeaderValue>) -> Result<Self> {
        Ok(PyOci {
            registry,
            transport: HttpTransport::new(auth)?,
        })
    }

    pub async fn list_package_versions<'a>(
        &mut self,
        package: &'a Package<'a, WithoutFile>,
    ) -> Result<Vec<String>> {
        let name = package.oci_name();
        let result = self.list_tags(&name).await?;
        tracing::debug!("{:?}", result);
        Ok(result.tags().to_owned())
    }
    /// List all files for the given package
    ///
    /// Limits the number of files to `n`
    /// ref: https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags
    pub async fn list_package_files<'a>(
        &mut self,
        package: &'a Package<'a, WithoutFile>,
        n: usize,
    ) -> Result<Vec<Package<'a, WithFile>>> {
        let name = package.oci_name();
        let result = self.list_tags(&name).await?;
        tracing::debug!("{:?}", result);
        let tags = result.tags();
        let mut files: Vec<Package<WithFile>> = Vec::new();
        let futures = FuturesUnordered::new();

        if tags.len() > n {
            tracing::warn!(
                "TagsList contains {} tags, only fetching the first {n}",
                tags.len()
            )
        }

        // We fetch a list of all tags from the OCI registry.
        // For each tag there can be multiple files.
        // We fetch the last `n` tags and for each tag we fetch the file names.
        // According to the spec the tags list should be in lexical order.
        // Even for non-spec registries the last-added seems to be at the end of the list
        // so this will result in the wanted list of tags in most cases.
        for tag in tags.iter().rev().take(n) {
            let pyoci = self.clone();
            futures.push(pyoci.package_info_for_ref(package, &name, tag));
        }
        for result in futures
            .collect::<Vec<Result<Vec<Package<WithFile>>, Error>>>()
            .await
        {
            files.append(&mut result?);
        }
        Ok(files)
    }

    async fn package_info_for_ref<'a>(
        mut self,
        package: &'a Package<'a, WithoutFile>,
        name: &str,
        reference: &str,
    ) -> Result<Vec<Package<'a, WithFile>>> {
        let manifest = self.pull_manifest(name, reference).await?;
        let index = match manifest {
            Some(Manifest::Index(index)) => index,
            Some(Manifest::Manifest(_)) => {
                bail!("Expected ImageIndex, got ImageManifest");
            }
            None => {
                return Err(PyOciError::from((
                    StatusCode::NOT_FOUND,
                    format!("ImageManifest '{reference}' does not exist"),
                ))
                .into())
            }
        };

        let artifact_type = index.artifact_type();
        match artifact_type {
            // Artifact type is as expected, do nothing
            Some(MediaType::Other(value)) if value == "application/pyoci.package.v1" => {}
            // Artifact type has unexpected value, err
            Some(value) => bail!("Unknown artifact type: {}", value),
            // Artifact type is not set, err
            None => bail!("No artifact type set"),
        };
        let mut files: Vec<Package<WithFile>> = Vec::new();
        for manifest in index.manifests() {
            match manifest.platform().as_ref().unwrap().architecture() {
                oci_spec::image::Arch::Other(arch) => {
                    let file = package.with_oci_file(reference, arch);
                    files.push(file);
                }
                arch => bail!("Unsupported architecture '{}'", arch),
            };
        }
        Ok(files)
    }

    pub async fn download_package_file(
        &mut self,
        package: &Package<'_, WithFile>,
    ) -> Result<Response> {
        // Pull index
        let index = match self
            .pull_manifest(&package.oci_name(), &package.oci_tag())
            .await?
        {
            Some(Manifest::Index(index)) => index,
            Some(Manifest::Manifest(_)) => {
                bail!("Expected ImageIndex, got ImageManifest");
            }
            None => {
                return Err(
                    PyOciError::from((StatusCode::NOT_FOUND, "ImageIndex does not exist")).into(),
                )
            }
        };
        // Check artifact type
        match index.artifact_type() {
            // Artifact type is as expected, do nothing
            Some(MediaType::Other(value)) if value == "application/pyoci.package.v1" => {}
            // Artifact type has unexpected value, err
            Some(value) => bail!("Unknown artifact type: {}", value),
            // Artifact type is not set, err
            None => bail!("No artifact type set"),
        };
        // Find manifest descriptor for platform
        let mut platform_manifest: Option<&oci_spec::image::Descriptor> = None;
        for manifest in index.manifests() {
            if let Some(platform) = manifest.platform() {
                match platform.architecture() {
                    oci_spec::image::Arch::Other(arch) if *arch == package.oci_architecture() => {
                        platform_manifest = Some(manifest);
                        break;
                    }
                    _ => {}
                }
            }
        }
        let manifest_descriptor = match platform_manifest {
            Some(descriptor) => descriptor,
            None => {
                return Err(PyOciError::from((
                    StatusCode::NOT_FOUND,
                    format!(
                        "Requested architecture '{}' not available",
                        package.oci_architecture()
                    ),
                ))
                .into())
            }
        };

        let manifest = match self
            .pull_manifest(&package.oci_name(), manifest_descriptor.digest().as_ref())
            .await?
        {
            Some(Manifest::Manifest(manifest)) => *manifest,
            Some(Manifest::Index(_)) => {
                bail!("Expected ImageManifest, got ImageIndex");
            }
            None => {
                return Err(PyOciError::from((
                    StatusCode::NOT_FOUND,
                    "ImageManifest does not exist",
                ))
                .into())
            }
        };
        // pull blob in first layer of manifest
        let [blob_descriptor] = &manifest.layers()[..] else {
            bail!("Image Manifest defines unexpected number of layers, was this package published by pyoci?");
        };
        self.pull_blob(package.oci_name(), blob_descriptor.to_owned())
            .await
    }

    pub async fn publish_package_file(
        &mut self,
        package: &Package<'_, WithFile>,
        file: Vec<u8>,
        mut annotations: HashMap<String, String>,
    ) -> Result<()> {
        let name = package.oci_name();
        let tag = package.oci_tag();

        let layer = Blob::new(file, ARTIFACT_TYPE);
        annotations.insert(
            "org.opencontainers.image.created".to_string(),
            OffsetDateTime::now_utc().format(&Rfc3339)?,
        );

        // Build the Manifest
        let config = empty_config();
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(config.descriptor.clone())
            .layers(vec![layer.descriptor.clone()])
            .annotations(annotations.clone())
            .build()
            .expect("valid ImageManifest");
        let manifest = PlatformManifest::new(manifest, package);
        // Pull an existing index
        let index = match self.pull_manifest(&name, &tag).await? {
            Some(Manifest::Manifest(_)) => {
                bail!("Expected ImageIndex, got ImageManifest");
            }
            Some(Manifest::Index(index)) => Some(index),
            None => None,
        };

        let index = match index {
            // No existing index found, create a new one
            None => ImageIndexBuilder::default()
                .schema_version(SCHEMA_VERSION)
                .media_type("application/vnd.oci.image.index.v1+json")
                .artifact_type(ARTIFACT_TYPE)
                .manifests(vec![manifest.descriptor(annotations.clone())])
                .annotations(annotations.clone())
                .build()
                .expect("valid ImageIndex"),
            // Existing index found, check artifact type
            Some(mut index) => {
                // Check artifact type
                match index.artifact_type() {
                    Some(MediaType::Other(value)) if value == ARTIFACT_TYPE => {}
                    Some(value) => bail!("Unknown artifact type: {}", value),
                    None => bail!("No artifact type set"),
                };
                for existing in index.manifests() {
                    match existing.platform() {
                        Some(platform) if *platform == manifest.platform => {
                            return Err(PyOciError::from((
                                StatusCode::CONFLICT,
                                format!(
                                    "Platform '{}' already exists for version '{}'",
                                    package.oci_architecture(),
                                    tag
                                ),
                            ))
                            .into())
                        }
                        _ => {}
                    }
                }
                let mut manifests = index.manifests().to_vec();
                manifests.push(manifest.descriptor(annotations));
                index.set_manifests(manifests);
                *index
            }
        };
        tracing::debug!("Index: {:?}", index.to_string().unwrap());
        tracing::debug!("Manifest: {:?}", manifest.manifest.to_string().unwrap());

        self.push_blob(&name, layer).await?;
        self.push_blob(&name, config).await?;
        self.push_manifest(&name, Manifest::Manifest(Box::new(manifest.manifest)), None)
            .await?;
        self.push_manifest(&name, Manifest::Index(Box::new(index)), Some(&tag))
            .await
    }

    pub async fn delete_package_version(&mut self, package: &Package<'_, WithFile>) -> Result<()> {
        let name = package.oci_name();
        let index = match self.pull_manifest(&name, &package.oci_tag()).await? {
            Some(Manifest::Index(index)) => index,
            Some(Manifest::Manifest(_)) => {
                bail!("Expected ImageIndex, got ImageManifest");
            }
            None => {
                return Err(
                    PyOciError::from((StatusCode::NOT_FOUND, "ImageIndex does not exist")).into(),
                )
            }
        };
        // Check artifact type
        match index.artifact_type() {
            // Artifact type is as expected, do nothing
            Some(MediaType::Other(value)) if value == "application/pyoci.package.v1" => {}
            // Artifact type has unexpected value, err
            Some(value) => bail!("Unknown artifact type: {}", value),
            // Artifact type is not set, err
            None => bail!("No artifact type set"),
        };
        for manifest in index.manifests() {
            let digest = manifest.digest().to_string();
            tracing::debug!("Deleting {name}:{digest}");
            self.delete_manifest(&name, &digest).await?
        }
        Ok(())
    }
}

impl PyOci {
    /// Push a blob to the registry using POST then PUT method
    ///
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#post-then-put
    #[tracing::instrument(skip_all, fields(otel.name = name))]
    async fn push_blob(
        &mut self,
        // Name of the package, including namespace. e.g. "library/alpine"
        name: &str,
        blob: Blob,
    ) -> Result<()> {
        let digest = blob.descriptor.digest().to_string();
        let response = self
            .transport
            .send(
                self.transport
                    .head(build_url!(&self, "/v2/{}/blobs/{}", name, &digest)),
            )
            .await?;

        match response.status() {
            StatusCode::OK => {
                tracing::info!("Blob already exists: {name}:{digest}");
                return Ok(());
            }
            StatusCode::NOT_FOUND => {}
            status => {
                return Err(PyOciError::from((status, response.text().await?)).into());
            }
        }

        let url = build_url!(&self, "/v2/{}/blobs/uploads/", name);
        let request = self
            .transport
            .post(url)
            .header("Content-Type", "application/octet-stream");
        let response = self.transport.send(request).await?;
        let location = match response.status() {
            StatusCode::CREATED => return Ok(()),
            StatusCode::ACCEPTED => response
                .headers()
                .get("Location")
                .context("Registry response did not contain a Location header")?
                .to_str()
                .context("Failed to parse Location header as ASCII")?,
            status => {
                return Err(PyOciError::from((status, response.text().await?)).into());
            }
        };
        let mut url: Url = build_url!(&self, "{}", location);
        // `append_pair` percent-encodes the values as application/x-www-form-urlencoded.
        // ghcr.io seems to be fine with a percent-encoded digest but this could be an issue with
        // other registries.
        url.query_pairs_mut().append_pair("digest", &digest);

        let request = self
            .transport
            .put(url)
            .header("Content-Type", "application/octet-stream")
            .header("Content-Length", blob.data.len().to_string())
            .body(blob.data);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::CREATED => {}
            status => {
                return Err(PyOciError::from((status, response.text().await?)).into());
            }
        }
        tracing::debug!(
            "Blob-location: {}",
            response
                .headers()
                .get("Location")
                .expect("valid Location header")
                .to_str()
                .expect("valid Location header value")
        );
        Ok(())
    }

    /// Pull a blob from the registry
    ///
    /// This returns the raw response so the caller can handle the blob as needed
    #[tracing::instrument(skip_all, fields(otel.name = name))]
    async fn pull_blob(
        &mut self,
        // Name of the package, including namespace. e.g. "library/alpine"
        name: String,
        // Descriptor of the blob to pull
        descriptor: Descriptor,
    ) -> Result<Response> {
        let digest = descriptor.digest().to_string();
        let url = build_url!(&self, "/v2/{}/blobs/{}", &name, &digest);
        let request = self.transport.get(url);
        let response = self.transport.send(request).await?;

        match response.status() {
            StatusCode::OK => Ok(response),
            status => Err(PyOciError::from((status, response.text().await?)).into()),
        }
    }

    /// List the available tags for a package
    #[tracing::instrument(skip_all, fields(otel.name = name))]
    async fn list_tags(&mut self, name: &str) -> anyhow::Result<TagList> {
        let url = build_url!(&self, "/v2/{}/tags/list", name);
        let request = self.transport.get(url);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::OK => {}
            status => return Err(PyOciError::from((status, response.text().await?)).into()),
        };
        let tags = response.json::<TagList>().await?;
        Ok(tags)
    }

    /// Push a manifest to the registry
    ///
    /// ImageIndex will be pushed with a version tag if version is set
    /// ImageManifest will always be pushed with a digest reference
    #[tracing::instrument(skip_all, fields(otel.name = name, otel.version = version))]
    async fn push_manifest(
        &mut self,
        name: &str,
        manifest: Manifest,
        version: Option<&str>,
    ) -> Result<()> {
        let (url, data, content_type) = match manifest {
            Manifest::Index(index) => {
                let version = version.context("`version` required for pushing an ImageIndex")?;
                let url = build_url!(&self, "v2/{}/manifests/{}", name, version);
                let data = serde_json::to_string(&index)?;
                (url, data, "application/vnd.oci.image.index.v1+json")
            }
            Manifest::Manifest(manifest) => {
                let data = serde_json::to_string(&manifest)?;
                let data_digest = digest(&data);
                let url = build_url!(&self, "/v2/{}/manifests/{}", name, data_digest.as_ref());
                (url, data, "application/vnd.oci.image.manifest.v1+json")
            }
        };

        let request = self
            .transport
            .put(url)
            .header("Content-Type", content_type)
            .body(data);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::CREATED => {}
            status => return Err(PyOciError::from((status, response.text().await?)).into()),
        };
        Ok(())
    }

    /// Pull a manifest from the registry
    ///
    /// If the manifest does not exist, Ok<None> is returned
    /// If any other error happens, an Err is returned
    #[tracing::instrument(skip_all, fields(otel.name = name, otel.reference = reference))]
    async fn pull_manifest(&mut self, name: &str, reference: &str) -> Result<Option<Manifest>> {
        let url = build_url!(&self, "/v2/{}/manifests/{}", name, reference);
        let request = self.transport.get(url).header(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        );
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(None),
            StatusCode::OK => {}
            status => return Err(PyOciError::from((status, response.text().await?)).into()),
        };

        match response.headers().get("Content-Type") {
            Some(value) if value == "application/vnd.oci.image.index.v1+json" => {
                Ok(Some(Manifest::Index(Box::new(
                    response
                        .json::<ImageIndex>()
                        .await
                        .expect("valid Index json"),
                ))))
            }
            Some(value) if value == "application/vnd.oci.image.manifest.v1+json" => {
                Ok(Some(Manifest::Manifest(Box::new(
                    response
                        .json::<ImageManifest>()
                        .await
                        .expect("valid Manifest json"),
                ))))
            }
            Some(content_type) => bail!("Unknown Content-Type: {}", content_type.to_str().unwrap()),
            None => bail!("Missing Content-Type header"),
        }
    }

    /// Delete a tag or manifest
    ///
    /// reference: tag or digest of the manifest to delete
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#content-management
    #[tracing::instrument(skip_all, fields(otel.name = name, otel.reference = reference))]
    async fn delete_manifest(&mut self, name: &str, reference: &str) -> Result<()> {
        let url = build_url!(&self, "/v2/{}/manifests/{}", name, reference);
        let request = self.transport.delete(url);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::ACCEPTED => Ok(()),
            status => Err(PyOciError::from((status, response.text().await?)).into()),
        }
    }
}

/// static EmptyConfig Descriptor
fn empty_config() -> Blob {
    Blob::new("{}".into(), "application/vnd.oci.empty.v1+json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url() -> Result<()> {
        let client = PyOci {
            registry: Url::parse("https://example.com").expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };
        let url = build_url!(&client, "/foo/{}/", "latest");
        assert_eq!(url.as_str(), "https://example.com/foo/latest/");
        Ok(())
    }

    #[test]
    fn test_build_url_absolute() -> Result<()> {
        let client = PyOci {
            registry: Url::parse("https://example.com").expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };
        let url = build_url!(&client, "{}/foo?bar=baz&qaz=sha:123", "http://pyoci.nl");
        assert_eq!(url.as_str(), "http://pyoci.nl/foo?bar=baz&qaz=sha:123");
        Ok(())
    }

    #[test]
    fn test_build_url_double_period() {
        let client = PyOci {
            registry: Url::parse("https://example.com").expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };
        let x = || -> Result<Url> { Ok(build_url!(&client, "/foo/{}/", "..")) }();
        assert!(x.is_err());
    }

    /// Test if a relative Location header is properly handled
    #[tokio::test]
    async fn test_push_blob_location_relative() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut mocks = vec![];
        // Mock the server, in order of expected requests

        // HEAD request to check if blob exists
        mocks.push(
            server
                .mock(
                    "HEAD",
                    "/v2/mockserver/foobar/blobs/sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
                )
                .with_status(404)
                .create_async()
                .await,
        );
        // POST request initiating blob upload
        mocks.push(
            server
                .mock("POST", "/v2/mockserver/foobar/blobs/uploads/")
                .with_status(202) // ACCEPTED
                .with_header(
                    "Location",
                    "/v2/mockserver/foobar/blobs/uploads/1?_state=uploading",
                )
                .create_async()
                .await,
        );
        // PUT request to upload blob
        mocks.push(
            server
                .mock(
                    "PUT",
                    "/v2/mockserver/foobar/blobs/uploads/1?_state=uploading&digest=sha256%3A2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
                )
                .with_status(201) // CREATED
                .create_async()
                .await,
        );

        let mut client = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };
        let blob = Blob::new("hello".into(), "application/octet-stream");
        let _ = client.push_blob("mockserver/foobar", blob).await;

        for mock in mocks {
            mock.assert_async().await;
        }
    }
    /// Test if an absolute Location header is properly handled
    #[tokio::test]
    async fn test_push_blob_location_absolute() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mut mocks = vec![];
        // Mock the server, in order of expected requests

        // HEAD request to check if blob exists
        mocks.push(
            server
                .mock(
                    "HEAD",
                    "/v2/mockserver/foobar/blobs/sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
                )
                .with_status(404)
                .create_async()
                .await,
        );
        // POST request initiating blob upload
        mocks.push(
            server
                .mock("POST", "/v2/mockserver/foobar/blobs/uploads/")
                .with_status(202) // ACCEPTED
                .with_header(
                    "Location",
                    &format!("{url}/v2/mockserver/foobar/blobs/uploads/1?_state=uploading"),
                )
                .create_async()
                .await,
        );
        // PUT request to upload blob
        mocks.push(
            server
                .mock(
                    "PUT",
                    "/v2/mockserver/foobar/blobs/uploads/1?_state=uploading&digest=sha256%3A2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
                )
                .with_status(201) // CREATED
                .create_async()
                .await,
        );

        let mut client = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };
        let blob = Blob::new("hello".into(), "application/octet-stream");
        let _ = client.push_blob("mockserver/foobar", blob).await;

        for mock in mocks {
            mock.assert_async().await;
        }
    }
}
