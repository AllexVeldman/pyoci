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
use serde_json::to_string_pretty;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::str::FromStr;
use time::format_description::well_known::Rfc3339;
use url::Url;

#[cfg(test)]
use crate::mocks::OffsetDateTime;
#[cfg(not(test))]
use time::OffsetDateTime;

use crate::package::{Package, WithFileName, WithoutFileName};
use crate::transport::{HttpTransport, Transport};
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
    fn new(manifest: ImageManifest, package: &Package<WithFileName>) -> Self {
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
#[derive(Debug)]
pub struct PyOci<T> {
    registry: Url,
    transport: T,
}

impl PyOci<HttpTransport> {
    /// Create a new Client
    pub fn new(registry: Url, auth: Option<HeaderValue>) -> Result<PyOci<HttpTransport>> {
        Ok(PyOci {
            registry,
            transport: HttpTransport::new(auth)?,
        })
    }
}

impl<T> Clone for PyOci<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            transport: self.transport.clone(),
        }
    }
}

/// Create/List/Download/Delete Packages
impl<T> PyOci<T>
where
    T: Transport + Clone,
{
    pub async fn list_package_versions<'a>(
        &mut self,
        package: &'a Package<'a, WithoutFileName>,
    ) -> Result<BTreeSet<String>> {
        let name = package.oci_name();
        let result = self.list_tags(&name).await?;
        tracing::debug!("{:?}", result);
        Ok(result)
    }

    /// List all files for the given package
    ///
    /// Limits the number of files to `n`
    /// ref: https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags
    pub async fn list_package_files<'a>(
        &mut self,
        package: &'a Package<'a, WithoutFileName>,
        mut n: usize,
    ) -> Result<Vec<Package<'a, WithFileName>>> {
        let tags = self.list_tags(&package.oci_name()).await?;
        let mut files: Vec<Package<WithFileName>> = Vec::new();
        let futures = FuturesUnordered::new();

        tracing::info!("# of tags: {}", tags.len());

        if n == 0 {
            // Fetch all versions
            n = tags.len()
        }
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
            futures.push(pyoci.package_info_for_ref(package, tag));
        }
        for result in futures
            .collect::<Vec<Result<Vec<Package<WithFileName>>, Error>>>()
            .await
        {
            files.append(&mut result?);
        }
        Ok(files)
    }

    pub async fn package_info_for_ref<'a>(
        mut self,
        package: &'a Package<'a, WithoutFileName>,
        reference: &str,
    ) -> Result<Vec<Package<'a, WithFileName>>> {
        let manifest = self.pull_manifest(&package.oci_name(), reference).await?;
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
        let mut files: Vec<Package<WithFileName>> = Vec::new();
        for manifest in index.manifests() {
            match manifest.platform().as_ref().unwrap().architecture() {
                oci_spec::image::Arch::Other(arch) => {
                    let mut sha256_digest = None;
                    let mut project_urls = None;
                    if let Some(annotations) = manifest.annotations() {
                        sha256_digest = annotations
                            .get("com.pyoci.sha256_digest")
                            .map(|v| v.to_string());
                        project_urls = annotations
                            .get("com.pyoci.project_urls")
                            .map(|v| v.to_string())
                    };
                    let file = package
                        .with_oci_file(reference, arch)
                        .with_sha256(sha256_digest)
                        .with_project_urls(project_urls);
                    files.push(file);
                }
                arch => bail!("Unsupported architecture '{}'", arch),
            };
        }
        Ok(files)
    }

    pub async fn download_package_file(
        &mut self,
        package: &Package<'_, WithFileName>,
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

    /// Construct and publish the manifests and blob provided.
    ///
    /// The `sha256_digest`, if provided, will be verified against the sha256 of the actual content.
    ///
    /// The `annotations` will be added to the ImageManifest, mimicking the default docker CLI
    /// behaviour.
    pub async fn publish_package_file(
        &mut self,
        package: &Package<'_, WithFileName>,
        file: Vec<u8>,
        mut annotations: HashMap<String, String>,
        sha256_digest: Option<String>,
        project_urls: HashMap<String, String>,
    ) -> Result<()> {
        let name = package.oci_name();
        let tag = package.oci_tag();

        let layer = Blob::new(file, ARTIFACT_TYPE);

        let package_digest = verify_digest(&layer, sha256_digest)?;

        // Annotations added to the manifest descriptor in the ImageIndex
        // We're adding the digest here so we don't need to pull the ImageManifest when listing
        // packages to get the package (blob) digest
        let mut index_manifest_annotations = HashMap::from([(
            "com.pyoci.sha256_digest".to_string(),
            package_digest.to_string(),
        )]);

        let creation_annotation = HashMap::from([(
            "org.opencontainers.image.created".to_string(),
            OffsetDateTime::now_utc().format(&Rfc3339)?,
        )]);

        annotations.extend(creation_annotation.clone());
        index_manifest_annotations.extend(creation_annotation.clone());
        index_manifest_annotations.insert(
            "com.pyoci.project_urls".to_string(),
            serde_json::to_string(&project_urls)?,
        );

        // Build the Manifest
        let manifest = self.image_manifest(package, &layer, annotations);
        let index = self
            .image_index(
                package,
                &manifest,
                creation_annotation,
                index_manifest_annotations,
            )
            .await?;
        tracing::debug!("{}", to_string_pretty(&index).unwrap());
        tracing::debug!("{}", to_string_pretty(&manifest.manifest).unwrap());

        self.push_blob(&name, layer).await?;
        self.push_blob(&name, empty_config()).await?;
        self.push_manifest(&name, Manifest::Manifest(Box::new(manifest.manifest)), None)
            .await?;
        self.push_manifest(&name, Manifest::Index(Box::new(index)), Some(&tag))
            .await
    }

    /// Get the definition of a new ImageManifest
    fn image_manifest(
        &self,
        package: &Package<'_, WithFileName>,
        layer: &Blob,
        annotations: HashMap<String, String>,
    ) -> PlatformManifest {
        let config = empty_config();
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(config.descriptor.clone())
            .layers(vec![layer.descriptor.clone()])
            .annotations(annotations)
            .build()
            .expect("valid ImageManifest");
        PlatformManifest::new(manifest, package)
    }

    /// Create or Update the definition of a new ImageIndex
    async fn image_index(
        &mut self,
        package: &Package<'_, WithFileName>,
        manifest: &PlatformManifest,
        index_annotations: HashMap<String, String>,
        index_manifest_annotations: HashMap<String, String>,
    ) -> Result<ImageIndex> {
        let name = package.oci_name();
        let tag = package.oci_tag();
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
                .manifests(vec![manifest.descriptor(index_manifest_annotations)])
                .annotations(index_annotations)
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
                manifests.push(manifest.descriptor(index_manifest_annotations));
                index.set_manifests(manifests);
                *index
            }
        };
        Ok(index)
    }

    pub async fn delete_package_version(
        &mut self,
        package: &Package<'_, WithFileName>,
    ) -> Result<()> {
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

/// Check if the provided digest matches the package digest
///
/// Returns the digest if successful
fn verify_digest(layer: &Blob, expected_digest: Option<String>) -> Result<String> {
    let package_digest = layer.descriptor.digest().digest();

    if let Some(sha256_digest) = expected_digest {
        // Verify if the sha256 as provided by the request matches the calculated sha of the
        // uploaded content.
        if package_digest != sha256_digest {
            Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "Provided sha256_digest does not match the package content",
            )))?;
        }
    }
    Ok(package_digest.to_string())
}

/// Low-level functionality for interacting with the OCI registry
impl<T> PyOci<T>
where
    T: Transport + Clone,
{
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
    ///
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags
    #[tracing::instrument(skip_all, fields(otel.name = name))]
    async fn list_tags(&mut self, name: &str) -> anyhow::Result<BTreeSet<String>> {
        let url = build_url!(&self, "/v2/{}/tags/list", name);
        let request = self.transport.get(url);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::OK => {}
            status => return Err(PyOciError::from((status, response.text().await?)).into()),
        };
        let mut link_header = match response.headers().get("link") {
            Some(link) => Some(Link::try_from(link)?),
            None => None,
        };
        let mut tags: BTreeSet<String> = response
            .json::<TagList>()
            .await?
            .tags()
            .iter()
            .map(|f| f.to_string())
            .collect();
        while let Some(ref link) = link_header {
            // Follow the link headers as long as a Link header is returned
            let mut url = self.registry.clone();
            url.set_path("");
            let url = url.join(&link.0)?;
            let request = self.transport.get(url);
            let response = self.transport.send(request).await?;
            match response.status() {
                StatusCode::OK => {}
                status => return Err(PyOciError::from((status, response.text().await?)).into()),
            };
            link_header = match response.headers().get("link") {
                Some(link) => Some(Link::try_from(link)?),
                None => None,
            };
            let tag_list = response.json::<TagList>().await?;
            tags.extend(tag_list.tags().iter().map(|f| f.to_string()));
        }

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

struct Link(String);

impl TryFrom<&HeaderValue> for Link {
    type Error = PyOciError;

    fn try_from(value: &HeaderValue) -> std::result::Result<Self, Self::Error> {
        let value = match value.to_str() {
            Ok(value) => value,
            _ => {
                return Err(PyOciError::from((
                    StatusCode::BAD_GATEWAY,
                    "OCI registry provided invalid Link header",
                )))
            }
        };
        let parts = value.split(';').collect::<Vec<_>>();
        tracing::debug!("{parts:?}");
        let target = match parts.first().map(|f| f.trim()) {
            Some(target) if target.starts_with('<') && target.ends_with('>') => {
                target.strip_prefix('<').unwrap().strip_suffix('>').unwrap()
            }
            _ => {
                return Err(PyOciError::from((
                    StatusCode::BAD_GATEWAY,
                    "OCI registry provided an invalid Link target",
                )))
            }
        };

        // Check the Link contains the correct "rel"
        let mut valid_rel = false;
        for param in &parts[1..] {
            match param.split_once('=') {
                Some((key, value)) if key.trim() == "rel" && value.trim() == "\"next\"" => {
                    valid_rel = true
                }
                _ => {}
            }
        }
        if !valid_rel {
            return Err(PyOciError::from((
                StatusCode::BAD_GATEWAY,
                "OCI registry provide invalid Link rel",
            )));
        }
        Ok(Link(target.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::from_str;

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

    #[test]
    // Check if the digest is returned when no expected digest is provided
    fn verify_digest_none() {
        let layer = Blob::new(vec![b'a', b'b', b'c'], "test-artifact");
        let sha = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string();
        let result = verify_digest(&layer, None).expect("SHAs should match");
        assert_eq!(result, sha);
    }

    #[test]
    // Check if the digest is returned when the expected digest matches
    fn verify_digest_match() {
        let layer = Blob::new(vec![b'a', b'b', b'c'], "test-artifact");
        let sha = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string();
        let result = verify_digest(&layer, Some(sha.clone())).expect("SHAs should match");
        assert_eq!(result, sha);
    }

    #[test]
    // Check if an error is returned if the sha does not match
    fn verify_digest_no_match() {
        let layer = Blob::new(vec![b'a', b'b', b'c'], "test-artifact");
        let result = verify_digest(&layer, Some("no-match".to_string()))
            .expect_err("Should return an error");
        let err = result
            .downcast::<PyOciError>()
            .expect("Error should be PyOciError");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_tags() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let tags = r#"{
          "name": "mockserver/bar",
          "tags": [
            "1",
            "2",
            "3"
          ]
        }"#;
        server
            .mock("GET", "/v2/mockserver/bar/tags/list")
            .with_status(200)
            .with_body(tags)
            .create_async()
            .await;

        let mut pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        let result = pyoci
            .list_tags("mockserver/bar")
            .await
            .expect("Valid response");

        assert_eq!(
            result,
            BTreeSet::from(["1".to_string(), "2".to_string(), "3".to_string()])
        );
    }

    #[tokio::test]
    async fn list_tags_link_header() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        server
            .mock("GET", "/v2/mockserver/bar/tags/list")
            .with_header(
                "Link",
                "</v2/mockserver/bar/tags/list?n=3&last=3>; rel=\"next\"",
            )
            .with_status(200)
            .with_body(
                r#"{
                  "name": "mockserver/bar",
                  "tags": [
                    "1",
                    "2",
                    "3"
                  ]
                }"#,
            )
            .create_async()
            .await;

        server
            .mock("GET", "/v2/mockserver/bar/tags/list?n=3&last=3")
            .with_header(
                "Link",
                "</v2/mockserver/bar/tags/list?n=3&last=6>; rel=\"next\"",
            )
            .with_status(200)
            .with_body(
                r#"{
                  "name": "mockserver/bar",
                  "tags": [
                    "4",
                    "5",
                    "6"
                  ]
                }"#,
            )
            .create_async()
            .await;

        server
            .mock("GET", "/v2/mockserver/bar/tags/list?n=3&last=6")
            .with_status(200)
            .with_body(
                r#"{
                  "name": "mockserver/bar",
                  "tags": [
                    "7"
                  ]
                }"#,
            )
            .create_async()
            .await;

        let mut pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        let result = pyoci
            .list_tags("mockserver/bar")
            .await
            .expect("Valid response");

        assert_eq!(
            result,
            BTreeSet::from([
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string(),
                "5".to_string(),
                "6".to_string(),
                "7".to_string(),
            ])
        );
    }

    #[tokio::test]
    async fn package_info_for_ref() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        // Existing ImageIndex
        let index = r#"{
          "schemaVersion": 2,
          "mediaType": "application/vnd.oci.image.index.v1+json",
          "artifactType": "application/pyoci.package.v1",
          "manifests": [
            {
              "mediaType": "application/vnd.oci.image.manifest.v1+json",
              "digest": "sha256:0d749abe1377573493e0df74df8d1282e46967754a1ebc7cc6323923a788ad5c",
              "size": 6,
              "platform": {
                "architecture": ".tar.gz",
                "os": "any"
              }
            }
          ],
          "annotations": {
            "created": "yesterday"
          }
        }"#;
        server
            .mock("GET", "/v2/mockserver/bar/manifests/1")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.index.v1+json")
            .with_body(index)
            .create_async()
            .await;

        let pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        let package = Package::new("ghcr.io", "mockserver", "bar");

        let result = pyoci
            .package_info_for_ref(&package, "1")
            .await
            .expect("Valid response");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].py_uri(), "/ghcr.io/mockserver/bar/bar-1.tar.gz");
    }

    #[tokio::test]
    async fn package_info_for_ref_sha256_digest() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        // Existing ImageIndex
        let index = r#"{
          "schemaVersion": 2,
          "mediaType": "application/vnd.oci.image.index.v1+json",
          "artifactType": "application/pyoci.package.v1",
          "manifests": [
            {
              "mediaType": "application/vnd.oci.image.manifest.v1+json",
              "digest": "sha256:0d749abe1377573493e0df74df8d1282e46967754a1ebc7cc6323923a788ad5c",
              "size": 6,
              "platform": {
                "architecture": ".tar.gz",
                "os": "any"
              },
              "annotations":{
                "com.pyoci.sha256_digest": "12345"
              }
            }
          ],
          "annotations": {
            "created": "yesterday"
          }
        }"#;
        server
            .mock("GET", "/v2/mockserver/bar/manifests/1")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.index.v1+json")
            .with_body(index)
            .create_async()
            .await;

        let pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        let package = Package::new("ghcr.io", "mockserver", "bar");

        let result = pyoci
            .package_info_for_ref(&package, "1")
            .await
            .expect("Valid response");

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].py_uri(),
            "/ghcr.io/mockserver/bar/bar-1.tar.gz#sha256=12345"
        );
    }

    #[test]
    fn image_manifest() {
        let pyoci = PyOci {
            registry: Url::parse("https://pyoci.com").expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        let package =
            Package::from_filename("ghcr.io", "mockserver", "bar-1.tar.gz").expect("Valid Package");
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let annotations = HashMap::from([(
            "test-annotation-key".to_string(),
            "test-annotation-value".to_string(),
        )]);

        let result = pyoci.image_manifest(&package, &layer, annotations.clone());
        assert_eq!(
            result.manifest,
            from_str::<ImageManifest>(r#"{
              "schemaVersion": 2,
              "mediaType": "application/vnd.oci.image.manifest.v1+json",
              "artifactType": "application/pyoci.package.v1",
              "config": {
                "mediaType": "application/vnd.oci.empty.v1+json",
                "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
                "size": 2
              },
              "layers": [
                {
                  "mediaType": "test-artifact",
                  "digest": "sha256:489cd5dbc708c7e541de4d7cd91ce6d0f1613573b7fc5b40d3942ccb9555cf35",
                  "size": 3
                }
              ],
              "annotations": {
                "test-annotation-key": "test-annotation-value"
              }
            }"#).unwrap()
        );
    }

    #[tokio::test]
    // Test if we can create a new ImageIndex
    async fn image_index_new() {
        // PyOci.image_index() will reach out to see if there is an existing index
        // Reply with a NOT_FOUND
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        server
            .mock("GET", "/v2/mockserver/bar/manifests/1")
            .with_status(404)
            .create_async()
            .await;

        let mut pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        // Setup the objects we're publishing
        let package = Package::from_filename("ghcr.io", "mockserver", "bar-1.tar.gz").unwrap();
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(empty_config().descriptor)
            .layers(vec![layer.descriptor])
            .build()
            .expect("valid ImageManifest");
        let manifest = PlatformManifest::new(manifest, &package);

        // Annotations for the ImageIndex
        let index_annotations = HashMap::from([("idx-key".to_string(), "idx-val".to_string())]);
        // Annotations for the ImageIndex.manifests[]
        let index_manifest_annotations =
            HashMap::from([("idx-mani-key".to_string(), "idx-mani-val".to_string())]);

        let result = pyoci
            .image_index(
                &package,
                &manifest,
                index_annotations,
                index_manifest_annotations,
            )
            .await
            .expect("Valid ImageIndex");

        assert_eq!(
            result,
            from_str::<ImageIndex>(r#"{
              "schemaVersion": 2,
              "mediaType": "application/vnd.oci.image.index.v1+json",
              "artifactType": "application/pyoci.package.v1",
              "manifests": [
                {
                  "mediaType": "application/vnd.oci.image.manifest.v1+json",
                  "digest": "sha256:6b95ce6324c6745397ccdb66864a73598b4df8989b1c0c8f0f386d85e2640d47",
                  "size": 406,
                  "annotations": {
                    "idx-mani-key": "idx-mani-val"
                  },
                  "platform": {
                    "architecture": ".tar.gz",
                    "os": "any"
                  }
                }
              ],
              "annotations": {
                "idx-key": "idx-val"
              }
            }"#).unwrap()
        );
    }

    #[tokio::test]
    // Test if we can update an existing ImageIndex
    async fn image_index_existing() {
        // PyOci.image_index() will reach out to see if there is an existing index
        // Reply with the existing index
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        // Existing ImageIndex
        let index = r#"{
          "schemaVersion": 2,
          "mediaType": "application/vnd.oci.image.index.v1+json",
          "artifactType": "application/pyoci.package.v1",
          "manifests": [
            {
              "mediaType": "application/vnd.oci.image.manifest.v1+json",
              "digest": "sha256:0d749abe1377573493e0df74df8d1282e46967754a1ebc7cc6323923a788ad5c",
              "size": 6,
              "platform": {
                "architecture": ".whl",
                "os": "any"
              }
            }
          ],
          "annotations": {
            "created": "yesterday"
          }
        }"#;

        server
            .mock("GET", "/v2/mockserver/bar/manifests/1")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.index.v1+json")
            .with_body(index)
            .create_async()
            .await;

        let mut pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        // Setup the objects we're publishing
        let package = Package::from_filename("ghcr.io", "mockserver", "bar-1.tar.gz").unwrap();
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(empty_config().descriptor)
            .layers(vec![layer.descriptor])
            .build()
            .expect("valid ImageManifest");
        let manifest = PlatformManifest::new(manifest, &package);

        // The ImageIndex annotations are only set when the index is newly created
        // So these annotations should not show up in the updated index
        let index_annotations = HashMap::from([("created".to_string(), "today".to_string())]);
        // Annotations for the new ImageIndex.manifests[]
        let index_manifest_annotations =
            HashMap::from([("idx-mani-key".to_string(), "idx-mani-val".to_string())]);

        let result = pyoci
            .image_index(
                &package,
                &manifest,
                index_annotations,
                index_manifest_annotations,
            )
            .await
            .expect("Valid ImageIndex");

        assert_eq!(
            result,
            from_str::<ImageIndex>(r#"{
              "schemaVersion": 2,
              "mediaType": "application/vnd.oci.image.index.v1+json",
              "artifactType": "application/pyoci.package.v1",
              "manifests": [
                {
                  "mediaType": "application/vnd.oci.image.manifest.v1+json",
                  "digest": "sha256:0d749abe1377573493e0df74df8d1282e46967754a1ebc7cc6323923a788ad5c",
                  "size": 6,
                  "platform": {
                    "architecture": ".whl",
                    "os": "any"
                  }
                },
                {
                  "mediaType": "application/vnd.oci.image.manifest.v1+json",
                  "digest": "sha256:6b95ce6324c6745397ccdb66864a73598b4df8989b1c0c8f0f386d85e2640d47",
                  "size": 406,
                  "annotations": {
                    "idx-mani-key": "idx-mani-val"
                  },
                  "platform": {
                    "architecture": ".tar.gz",
                    "os": "any"
                  }
                }
              ],
              "annotations": {
                "created": "yesterday"
              }
            }"#).unwrap()
        );
    }

    #[tokio::test]
    // Test if existing packages are rejected
    async fn image_index_conflict() {
        // PyOci.image_index() will reach out to see if there is an existing index
        // Reply with the existing index
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        // Existing ImageIndex
        let index = r#"{
          "schemaVersion": 2,
          "mediaType": "application/vnd.oci.image.index.v1+json",
          "artifactType": "application/pyoci.package.v1",
          "manifests": [
            {
              "mediaType": "application/vnd.oci.image.manifest.v1+json",
              "digest": "sha256:6b95ce6324c6745397ccdb66864a73598b4df8989b1c0c8f0f386d85e2640d47",
              "size": 406,
              "annotations": {
                "idx-mani-key": "idx-mani-val"
              },
              "platform": {
                "architecture": ".tar.gz",
                "os": "any"
              }
            }
          ],
          "annotations": {
            "created": "yesterday"
          }
        }"#;

        server
            .mock("GET", "/v2/mockserver/bar/manifests/1")
            .with_status(200)
            .with_header("content-type", "application/vnd.oci.image.index.v1+json")
            .with_body(index)
            .create_async()
            .await;

        let mut pyoci = PyOci {
            registry: Url::parse(&url).expect("valid url"),
            transport: HttpTransport::new(None).unwrap(),
        };

        // Setup the objects we're publishing
        let package = Package::from_filename("ghcr.io", "mockserver", "bar-1.tar.gz").unwrap();
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(empty_config().descriptor)
            .layers(vec![layer.descriptor])
            .build()
            .expect("valid ImageManifest");
        let manifest = PlatformManifest::new(manifest, &package);

        let result = pyoci
            .image_index(&package, &manifest, HashMap::new(), HashMap::new())
            .await
            .expect_err("Expected an Err")
            .downcast::<PyOciError>()
            .expect("Expected a PyOciError");

        assert_eq!(result.status, StatusCode::CONFLICT);
        assert_eq!(
            result.message,
            "Platform '.tar.gz' already exists for version '1'"
        );
    }

    #[test]
    fn link() {
        let link = Link::try_from(&HeaderValue::from_static("</v2/allexveldman/hello_world/tags/list?last=0.0.1-example.1.poetry.2824051&n=5>; rel=\"next\"")).unwrap();
        assert_eq!(
            link.0,
            "/v2/allexveldman/hello_world/tags/list?last=0.0.1-example.1.poetry.2824051&n=5"
        )
    }
}
