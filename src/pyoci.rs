use anyhow::{bail, Error, Result};
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use http::HeaderValue;
use http::StatusCode;
use oci_spec::image::{
    ImageIndex, ImageIndexBuilder, ImageManifestBuilder, MediaType, SCHEMA_VERSION,
};
use reqwest::Response;
use serde_json::to_string_pretty;
use std::collections::BTreeSet;
use std::collections::HashMap;
use time::format_description::well_known::Rfc3339;
use url::Url;

use crate::error::PyOciError;
use crate::oci::Blob;
use crate::oci::Manifest;
use crate::oci::Oci;
use crate::oci::PlatformManifest;
use crate::time::now_utc;

use crate::package::{Package, WithFileName, WithoutFileName};
use crate::ARTIFACT_TYPE;

/// Client to communicate with the OCI v2 registry
#[derive(Debug, Clone)]
pub struct PyOci {
    oci: Oci,
}

impl PyOci {
    /// Create a new Client
    pub fn new(registry: Url, auth: Option<HeaderValue>) -> PyOci {
        PyOci {
            oci: Oci::new(registry, auth),
        }
    }
}

/// Create/List/Download/Delete Packages
impl PyOci {
    pub async fn list_package_versions<'a>(
        &mut self,
        package: &'a Package<'a, WithoutFileName>,
    ) -> Result<BTreeSet<String>> {
        let name = package.oci_name();
        let result = self.oci.list_tags(&name).await?;
        tracing::debug!("{:?}", result);
        Ok(result)
    }

    /// List all files for the given package
    ///
    /// Limits the number of files to `n`
    /// ref: <https://github.com/opencontainers/distribution-spec/blob/main/spec.md#listing-tags>
    pub async fn list_package_files<'a>(
        &mut self,
        package: &'a Package<'a, WithoutFileName>,
        mut n: usize,
    ) -> Result<Vec<Package<'a, WithFileName>>> {
        let tags = self.oci.list_tags(&package.oci_name()).await?;
        let mut files: Vec<Package<WithFileName>> = Vec::new();
        let futures = FuturesUnordered::new();

        tracing::info!("# of tags: {}", tags.len());

        if n == 0 {
            // Fetch all versions
            n = tags.len();
        }
        if tags.len() > n {
            tracing::warn!(
                "TagsList contains {} tags, only fetching the first {n}",
                tags.len()
            );
        }

        // We fetch a list of all tags from the OCI registry.
        // For each tag there can be multiple files.
        // We fetch the last `n` tags and for each tag we fetch the file names.
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

    /// Fetch all files for a single version of a package
    pub async fn package_info_for_ref<'a>(
        mut self,
        package: &'a Package<'a, WithoutFileName>,
        reference: &str,
    ) -> Result<Vec<Package<'a, WithFileName>>> {
        let manifest = self
            .oci
            .pull_manifest(&package.oci_name(), reference)
            .await?;
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
            Some(value) => bail!("Unknown artifact type: {value}"),
            // Artifact type is not set, err
            None => bail!("No artifact type set"),
        }
        let mut files: Vec<Package<WithFileName>> = Vec::new();
        for manifest in index.manifests() {
            match manifest.platform().as_ref().unwrap().architecture() {
                oci_spec::image::Arch::Other(arch) => {
                    let mut sha256_digest = None;
                    let mut project_urls = None;
                    if let Some(annotations) = manifest.annotations() {
                        sha256_digest = annotations
                            .get("com.pyoci.sha256_digest")
                            .map(ToString::to_string);
                        project_urls = annotations
                            .get("com.pyoci.project_urls")
                            .map(ToString::to_string);
                    }
                    let file = package
                        .with_oci_file(reference, arch)
                        .with_sha256(sha256_digest)
                        .with_project_urls(project_urls);
                    files.push(file);
                }
                arch => bail!("Unsupported architecture '{arch}'"),
            }
        }
        Ok(files)
    }

    /// Download a single file of a package
    pub async fn download_package_file(
        &mut self,
        package: &Package<'_, WithFileName>,
    ) -> Result<Response> {
        // Pull index
        let index = match self
            .oci
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
            Some(value) => bail!("Unknown artifact type: {value}"),
            // Artifact type is not set, err
            None => bail!("No artifact type set"),
        }
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
        let Some(manifest_descriptor) = platform_manifest else {
            return Err(PyOciError::from((
                StatusCode::NOT_FOUND,
                format!(
                    "Requested architecture '{}' not available",
                    package.oci_architecture()
                ),
            ))
            .into());
        };

        let manifest = match self
            .oci
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
        self.oci
            .pull_blob(package.oci_name(), blob_descriptor.to_owned())
            .await
    }

    /// Publish a package file
    ///
    /// Constructs and publishes the manifests and file data provided.
    ///
    /// The `sha256_digest`, if provided, will be verified against the sha256 of the actual content.
    ///
    /// The `annotations` will be added to the `ImageManifest`, mimicking the default docker CLI
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
        let mut index_manifest_annotations =
            HashMap::from([("com.pyoci.sha256_digest".to_string(), package_digest)]);

        let creation_annotation = HashMap::from([(
            "org.opencontainers.image.created".to_string(),
            now_utc().format(&Rfc3339)?,
        )]);

        annotations.extend(creation_annotation.clone());
        index_manifest_annotations.extend(creation_annotation.clone());
        index_manifest_annotations.insert(
            "com.pyoci.project_urls".to_string(),
            serde_json::to_string(&project_urls)?,
        );

        // Build the Manifest
        let manifest = image_manifest(package, &layer, annotations);
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

        self.oci.push_blob(&name, layer).await?;
        self.oci.push_blob(&name, empty_config()).await?;
        self.oci
            .push_manifest(&name, Manifest::Manifest(Box::new(manifest.manifest)), None)
            .await?;
        self.oci
            .push_manifest(&name, Manifest::Index(Box::new(index)), Some(&tag))
            .await
    }

    /// Create or Update the definition of a new `ImageIndex`
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
        let index = match self.oci.pull_manifest(&name, &tag).await? {
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
                    Some(value) => bail!("Unknown artifact type: {value}"),
                    None => bail!("No artifact type set"),
                }
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
                let mut manifests = index.manifests().clone();
                manifests.push(manifest.descriptor(index_manifest_annotations));
                index.set_manifests(manifests);
                *index
            }
        };
        Ok(index)
    }

    /// Delete a package version
    pub async fn delete_package_version(
        &mut self,
        package: &Package<'_, WithFileName>,
    ) -> Result<()> {
        let name = package.oci_name();
        let index = match self.oci.pull_manifest(&name, &package.oci_tag()).await? {
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
            Some(value) => bail!("Unknown artifact type: {value}"),
            // Artifact type is not set, err
            None => bail!("No artifact type set"),
        }
        for manifest in index.manifests() {
            let digest = manifest.digest().to_string();
            tracing::debug!("Deleting {name}:{digest}");
            self.oci.delete_manifest(&name, &digest).await?;
        }
        Ok(())
    }
}

/// Get the definition of a new `ImageManifest`
fn image_manifest(
    package: &Package<'_, WithFileName>,
    layer: &Blob,
    annotations: HashMap<String, String>,
) -> PlatformManifest {
    let config = empty_config();
    let manifest = ImageManifestBuilder::default()
        .schema_version(SCHEMA_VERSION)
        .media_type("application/vnd.oci.image.manifest.v1+json")
        .artifact_type(ARTIFACT_TYPE)
        .config(config.descriptor().clone())
        .layers(vec![layer.descriptor().clone()])
        .annotations(annotations)
        .build()
        .expect("valid ImageManifest");
    PlatformManifest::new(manifest, package)
}

/// Check if the provided digest matches the package digest
///
/// Returns the digest if successful
fn verify_digest(layer: &Blob, expected_digest: Option<String>) -> Result<String> {
    let package_digest = layer.descriptor().digest().digest();

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

/// static `EmptyConfig` Descriptor
fn empty_config() -> Blob {
    Blob::new("{}".into(), "application/vnd.oci.empty.v1+json")
}

#[cfg(test)]
mod tests {
    use oci_spec::image::ImageManifest;
    use pretty_assertions::assert_eq;
    use serde_json::from_str;

    use super::*;

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
            oci: Oci::new(Url::parse(&url).expect("valid url"), None),
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
            oci: Oci::new(Url::parse(&url).expect("valid url"), None),
        };

        let package = Package::new("ghcr.io", "mockserver", "bar");

        let result = pyoci
            .package_info_for_ref(&package, "1")
            .await
            .expect("Valid response");

        assert_eq!(result.len(), 1);
        assert_eq!(
            serde_json::to_string(&result).unwrap(),
            r#"[{"py_uri":"/ghcr.io/mockserver/bar/bar-1.tar.gz","filename":"bar-1.tar.gz","sha256":"12345"}]"#
        );
    }

    #[test]
    fn image_manifest() {
        let package = Package::from_filename("ghcr.io", "mockserver", "bar", "bar-1.tar.gz")
            .expect("Valid Package");
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let annotations = HashMap::from([(
            "test-annotation-key".to_string(),
            "test-annotation-value".to_string(),
        )]);

        let result = super::image_manifest(&package, &layer, annotations.clone());
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
            oci: Oci::new(Url::parse(&url).expect("valid url"), None),
        };

        // Setup the objects we're publishing
        let package =
            Package::from_filename("ghcr.io", "mockserver", "bar", "bar-1.tar.gz").unwrap();
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(empty_config().descriptor().to_owned())
            .layers(vec![layer.descriptor().to_owned()])
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
            oci: Oci::new(Url::parse(&url).expect("valid url"), None),
        };

        // Setup the objects we're publishing
        let package =
            Package::from_filename("ghcr.io", "mockserver", "bar", "bar-1.tar.gz").unwrap();
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(empty_config().descriptor().clone())
            .layers(vec![layer.descriptor().clone()])
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
            oci: Oci::new(Url::parse(&url).expect("valid url"), None),
        };

        // Setup the objects we're publishing
        let package =
            Package::from_filename("ghcr.io", "mockserver", "bar", "bar-1.tar.gz").unwrap();
        let layer = Blob::new(vec![b'q', b'w', b'e'], "test-artifact");
        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type(ARTIFACT_TYPE)
            .config(empty_config().descriptor().clone())
            .layers(vec![layer.descriptor().clone()])
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
}
