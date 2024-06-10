use anyhow::{bail, Context, Error, Result};
use base16ct::lower::encode_string as hex_encode;
use base64::prelude::*;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use oci_spec::image::Arch;
use oci_spec::image::DescriptorBuilder;
use oci_spec::image::ImageIndexBuilder;
use oci_spec::image::ImageManifestBuilder;
use oci_spec::image::Os;
use oci_spec::image::Platform;
use oci_spec::image::PlatformBuilder;
use oci_spec::image::SCHEMA_VERSION;
use oci_spec::{
    distribution::{ErrorResponse, TagList},
    image::{Descriptor, ImageIndex, ImageManifest, MediaType},
};
use regex::Regex;
use reqwest::Response;
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::package;
use crate::transport::HttpTransport;
use crate::ARTIFACT_TYPE;

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
    fn new(manifest: ImageManifest, package: &package::Info) -> Self {
        let platform = PlatformBuilder::default()
            .architecture(Arch::Other(package.file.architecture()))
            .os(Os::Other("any".to_string()))
            .build()
            .expect("valid Platform");
        PlatformManifest { manifest, platform }
    }

    fn descriptor(&self) -> Descriptor {
        let (digest, data) = self.digest();
        DescriptorBuilder::default()
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .digest(digest)
            .size(data.len() as i64)
            // Embed the content of the manifest in it's Descriptor
            // This would put the entire content of the manifest in the ImageIndex
            // saving a roundtrip to the registry when pulling the package
            // .data(BASE64_STANDARD.encode(data.as_bytes()))
            .platform(self.platform.clone())
            .build()
            .expect("Valid PlatformManifest Descriptor")
    }

    fn digest(&self) -> (String, String) {
        let data = serde_json::to_string(&self.manifest).expect("valid json");
        (digest(data.as_bytes()), data)
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
            .size(data.len() as i64)
            .build()
            .expect("valid Descriptor");
        Blob { data, descriptor }
    }
}

fn digest(data: &[u8]) -> String {
    let sha = <Sha256 as Digest>::digest(data);
    format!("sha256:{}", hex_encode(&sha))
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
    /// Parse a WWW-Authenticate header
    pub fn parse(value: &str) -> Result<Self> {
        let value = match value.strip_prefix("Bearer ") {
            None => bail!("Not a Bearer token"),
            Some(value) => value,
        };
        let realm = match Regex::new(r#"realm="(?P<realm>[^"\s]*)"#)
            .unwrap()
            .captures(value)
        {
            Some(value) => value.name("realm").unwrap().as_str().to_string(),
            None => bail!("`realm` key missing from WWW-Authenticate header"),
        };
        let service = match Regex::new(r#"service="(?P<service>[^"\s]*)"#)
            .expect("valid regex")
            .captures(value)
        {
            Some(value) => value.name("service").unwrap().as_str().to_string(),
            None => bail!("`service` key missing from WWW-Authenticate header"),
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
    /// Limits the number of files to `n`
    /// ref: https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags
    pub async fn list_package_files(
        &self,
        package: &package::Info,
        n: usize,
    ) -> Result<Vec<package::Info>> {
        let result = self.list_tags(&package.oci_name()).await?;
        tracing::debug!("{:?}", result);
        let tags = result.tags();
        let name = result.name();
        let mut files: Vec<package::Info> = Vec::new();
        let futures = FuturesUnordered::new();

        // We fetch a list of all tags from the OCI registry.
        // For each tag there can be multiple files.
        // We fetch the last `n` tags and for each tag we fetch the file names.
        // According to the spec the tags list should be in lexical order.
        // Even for non-spec registries the last-added seems to be at the end of the list
        // so this will result in the wanted list of tags in most cases.
        for tag in tags.iter().rev().take(n) {
            futures.push(self.package_info_for_ref(package, name, tag));
        }
        for result in futures
            .collect::<Vec<Result<Vec<package::Info>, Error>>>()
            .await
        {
            files.append(&mut result?);
        }
        Ok(files)
    }

    async fn package_info_for_ref(
        &self,
        package: &package::Info,
        name: &str,
        reference: &str,
    ) -> Result<Vec<package::Info>> {
        let manifest = self.pull_manifest(name, reference).await?;
        let Manifest::Index(index) = manifest else {
            bail!("Expected ImageIndex, got ImageManifest");
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
        let mut files: Vec<package::Info> = Vec::new();
        for manifest in index.manifests() {
            match manifest.platform().as_ref().unwrap().architecture() {
                oci_spec::image::Arch::Other(arch) => {
                    let mut file = package.clone();
                    file.file = package
                        .file
                        .clone()
                        .with_version(reference)
                        .with_architecture(arch)?;
                    files.push(file);
                }
                arch => bail!("Unsupported architecture '{}'", arch),
            };
        }
        Ok(files)
    }

    pub async fn download_package_file(&self, package: &crate::package::Info) -> Result<Response> {
        if !package.file.is_valid() {
            bail!("Not a valid package file: {}", package.file);
        };
        // Pull index
        let index = match self
            .pull_manifest(&package.oci_name(), &package.file.version)
            .await?
        {
            Manifest::Index(index) => index,
            Manifest::Manifest(_) => {
                bail!("Expected ImageIndex, got ImageManifest");
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
                    oci_spec::image::Arch::Other(arch) if *arch == package.file.architecture() => {
                        platform_manifest = Some(manifest);
                        break;
                    }
                    _ => {}
                }
            }
        }
        let manifest_descriptor = platform_manifest.with_context(|| {
            format!(
                "Requested architecture '{}' not available",
                package.file.architecture()
            )
        })?;
        let manifest = match manifest_descriptor.data() {
            Some(data) => {
                // Manifest is embedded in the index
                let data = BASE64_STANDARD.decode(data).expect("valid base64");
                serde_json::from_slice::<ImageManifest>(&data).expect("valid json")
            }
            None => {
                // pull manifest
                match self
                    .pull_manifest(&package.oci_name(), manifest_descriptor.digest())
                    .await?
                {
                    Manifest::Index(_) => {
                        bail!("Expected ImageManifest, got ImageIndex");
                    }
                    Manifest::Manifest(manifest) => *manifest,
                }
            }
        };
        // pull blob in first layer of manifest
        let [blob_descriptor] = &manifest.layers()[..] else {
            bail!("Manifest should define exactly one layer");
        };
        self.pull_blob(package.oci_name(), blob_descriptor.to_owned())
            .await
    }

    pub async fn publish_package_file(
        &self,
        package: &crate::package::Info,
        file: Vec<u8>,
    ) -> Result<()> {
        if !package.file.is_valid() {
            bail!("Not a valid package file: {}", package.file);
        };

        let name = package.oci_name();

        let layer = Blob::new(file, ARTIFACT_TYPE);

        // Build the Manifest
        let config = empty_config();
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(config.descriptor.clone())
            .layers(vec![layer.descriptor.clone()])
            .build()
            .expect("valid ImageManifest");
        let manifest = PlatformManifest::new(manifest, package);
        // Pull an existing index
        let index = match self.pull_manifest(&name, &package.file.version).await {
            Ok(Manifest::Manifest(_)) => {
                bail!("Expected ImageIndex, got ImageManifest");
            }
            Ok(Manifest::Index(index)) => Some(index),
            // TODO: This swallows 404 and all other errors, only 404 is expected
            Err(_) => None,
        };

        let index = match index {
            // No existing index found, create a new one
            None => ImageIndexBuilder::default()
                .schema_version(SCHEMA_VERSION)
                .media_type("application/vnd.oci.image.index.v1+json")
                .artifact_type(ARTIFACT_TYPE)
                .manifests(vec![manifest.descriptor()])
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
                            bail!(
                                "Platform '{}' already exists for version '{}'",
                                package.file.architecture(),
                                package.file.version
                            );
                        }
                        _ => {}
                    }
                }
                let mut manifests = index.manifests().to_vec();
                manifests.push(manifest.descriptor());
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
        self.push_manifest(
            &name,
            Manifest::Index(Box::new(index)),
            Some(&package.file.version),
        )
        .await
    }
}

impl PyOci {
    /// Push a blob to the registry using POST then PUT method
    ///
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#post-then-put
    async fn push_blob(
        &self,
        // Name of the package, including namespace. e.g. "library/alpine"
        name: &str,
        blob: Blob,
    ) -> Result<()> {
        let digest = blob.descriptor.digest();
        let response = self
            .transport
            .send(
                self.transport
                    .head(self.build_url(&format!("/v2/{name}/blobs/{digest}"))),
            )
            .await
            .expect("valid response");

        if response.status() == StatusCode::OK {
            tracing::info!("Blob already exists: {name}:{digest}");
            return Ok(());
        }

        let url = self.build_url(&format!("/v2/{name}/blobs/uploads/"));
        let request = self
            .transport
            .post(url)
            .header("Content-Type", "application/octet-stream");
        let response = self.transport.send(request).await.expect("valid response");
        let location = match response.status() {
            StatusCode::CREATED => return Ok(()),
            StatusCode::ACCEPTED => response
                .headers()
                .get("Location")
                .expect("a Location header")
                .to_str()
                .expect("valid Location header value"),
            _ => {
                bail!(response.json::<ErrorResponse>().await?)
            }
        };
        let mut url: Url = if location.starts_with('/') {
            self.build_url(location)
        } else {
            location.parse().expect("valid url")
        };
        url.query_pairs_mut().append_pair("digest", digest);

        let request = self
            .transport
            .put(url)
            .header("Content-Type", "application/octet-stream")
            .header("Content-Length", blob.data.len().to_string())
            .body(blob.data);
        let response = self.transport.send(request).await.expect("valid response");
        if response.status() != StatusCode::CREATED {
            bail!(response.json::<ErrorResponse>().await?)
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
    async fn pull_blob(
        &self,
        // Name of the package, including namespace. e.g. "library/alpine"
        name: String,
        // Descriptor of the blob to pull
        descriptor: Descriptor,
    ) -> Result<Response> {
        let digest = descriptor.digest();
        let url = self.build_url(&format!("/v2/{name}/blobs/{digest}"));
        let request = self.transport.get(url);
        let response = self.transport.send(request).await.expect("valid response");

        if !response.status().is_success() {
            bail!(response.json::<ErrorResponse>().await?)
        };
        Ok(response)
    }
    async fn list_tags(&self, name: &str) -> Result<TagList, Error> {
        let url = self.build_url(&format!("/v2/{name}/tags/list"));
        let request = self.transport.get(url);
        let response = self.transport.send(request).await.expect("valid response");
        if !response.status().is_success() {
            bail!(response.json::<ErrorResponse>().await?)
        };
        let tags = response
            .json::<TagList>()
            .await
            .expect("valid TagList json");
        Ok(tags)
    }

    /// Push a manifest to the registry
    ///
    /// ImageIndex will be pushed with a version tag if version is set
    /// ImageManifest will always be pushed with a digest reference
    async fn push_manifest(
        &self,
        name: &str,
        manifest: Manifest,
        version: Option<&str>,
    ) -> Result<()> {
        let (url, data, content_type) = match manifest {
            Manifest::Index(index) => {
                let version = version.context("`version` required for pushing and ImageIndex")?;
                let url = self.build_url(&format!("/v2/{name}/manifests/{version}"));
                let data = index.to_string().expect("valid json");
                (url, data, "application/vnd.oci.image.index.v1+json")
            }
            Manifest::Manifest(manifest) => {
                let data = manifest.to_string().expect("valid json");
                let sha = <Sha256 as Digest>::digest(&data);
                let digest = format!("sha256:{}", hex_encode(&sha));
                let url = self.build_url(&format!("/v2/{name}/manifests/{digest}"));
                (url, data, "application/vnd.oci.image.manifest.v1+json")
            }
        };

        let request = self
            .transport
            .put(url)
            .header("Content-Type", content_type)
            .body(data);
        let response = self.transport.send(request).await.expect("valid response");
        if !response.status().is_success() {
            bail!(response.json::<ErrorResponse>().await?)
        };
        Ok(())
    }

    async fn pull_manifest(&self, name: &str, reference: &str) -> Result<Manifest> {
        let url = self.build_url(&format!("/v2/{name}/manifests/{reference}"));
        let request = self.transport.get(url).header(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        );
        let response = self.transport.send(request).await.expect("valid response");
        if !response.status().is_success() {
            bail!(response.json::<ErrorResponse>().await?)
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
            Some(content_type) => bail!("Unknown Content-Type: {}", content_type.to_str().unwrap()),
            None => bail!("Missing Content-Type header"),
        }
    }
}

/// static EmptyConfig Descriptor
fn empty_config() -> Blob {
    Blob::new("{}".into(), "application/vnd.oci.empty.v1+json")
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
