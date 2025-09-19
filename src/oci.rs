use std::{
    collections::{BTreeSet, HashMap},
    str::FromStr,
};

use anyhow::{bail, Context, Result};
use base16ct::lower::encode_string as hex_encode;
use http::{HeaderValue, StatusCode};
use oci_spec::{
    distribution::TagList,
    image::{
        Arch, Descriptor, DescriptorBuilder, Digest as OciDigest, ImageIndex, ImageManifest, Os,
        Platform, PlatformBuilder, Sha256Digest,
    },
};
use reqwest::Response;
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    error::PyOciError,
    package::{Package, WithFileName},
    transport::HttpTransport,
};

/// Build an URL from a format string while sanitizing the parameters
///
/// Note that if the resulting path is an absolute URL, the registry URL is ignored.
/// For more info, see [`Url::join`]
///
/// Returns Err when a parameter fails sanitization
macro_rules! build_url {
    ($url:expr, $uri:literal, $($param:expr),+) => {{
            let uri = format!(
                $uri,
                $(sanitize($param)?,)*
            );
            let mut new_url = $url.clone();
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

/// Container for a Blob/Layer data, combined with a Descriptor
pub struct Blob {
    data: Vec<u8>,
    descriptor: Descriptor,
}

impl Blob {
    pub fn new(data: Vec<u8>, artifact_type: &str) -> Self {
        let digest = digest(&data);
        let descriptor = DescriptorBuilder::default()
            .media_type(artifact_type)
            .digest(digest)
            .size(data.len() as u64)
            .build()
            .expect("valid Descriptor");
        Blob { data, descriptor }
    }

    pub fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}

/// Calculate the digest of the provided data
pub fn digest(data: impl AsRef<[u8]>) -> OciDigest {
    let sha = <Sha256 as Digest>::digest(data);
    Sha256Digest::from_str(&hex_encode(&sha))
        .expect("Invalid Digest")
        .into()
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
pub struct PlatformManifest {
    pub manifest: ImageManifest,
    pub platform: Platform,
}

impl PlatformManifest {
    pub fn new(manifest: ImageManifest, package: &Package<WithFileName>) -> Self {
        let platform = PlatformBuilder::default()
            .architecture(Arch::Other(package.oci_architecture().to_string()))
            .os(Os::Other("any".to_string()))
            .build()
            .expect("valid Platform");
        PlatformManifest { manifest, platform }
    }

    pub fn descriptor(&self, annotations: HashMap<String, String>) -> Descriptor {
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

/// Implements the client side of the OCI distribution specification
#[derive(Debug, Clone)]
pub struct Oci {
    registry: Url,
    transport: HttpTransport,
}

/// Low-level functionality for interacting with the OCI registry
impl Oci {
    pub fn new(registry: Url, auth: Option<HeaderValue>) -> Result<Oci> {
        Ok(Oci {
            registry,
            transport: HttpTransport::new(auth)?,
        })
    }
    /// Push a blob to the registry using POST then PUT method
    ///
    /// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#post-then-put
    #[tracing::instrument(skip_all, fields(otel.name = name))]
    pub async fn push_blob(
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
                    .head(build_url!(&self.registry, "/v2/{}/blobs/{}", name, &digest)),
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

        let url = build_url!(&self.registry, "/v2/{}/blobs/uploads/", name);
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
        let mut url: Url = build_url!(&self.registry, "{}", location);
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
    pub async fn pull_blob(
        &mut self,
        // Name of the package, including namespace. e.g. "library/alpine"
        name: String,
        // Descriptor of the blob to pull
        descriptor: Descriptor,
    ) -> Result<Response> {
        let digest = descriptor.digest().to_string();
        let url = build_url!(&self.registry, "/v2/{}/blobs/{}", &name, &digest);
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
    pub async fn list_tags(&mut self, name: &str) -> anyhow::Result<BTreeSet<String>> {
        let url = build_url!(&self.registry, "/v2/{}/tags/list", name);
        let request = self.transport.get(url);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::OK => {}
            status => return Err(PyOciError::from((status, response.text().await?)).into()),
        }
        let mut link_header = match response.headers().get("link") {
            Some(link) => Some(Link::try_from(link)?),
            None => None,
        };
        let mut tags: BTreeSet<String> = response
            .json::<TagList>()
            .await?
            .tags()
            .iter()
            .map(|f| f.to_owned())
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
            }
            link_header = match response.headers().get("link") {
                Some(link) => Some(Link::try_from(link)?),
                None => None,
            };
            let tag_list = response.json::<TagList>().await?;
            tags.extend(tag_list.tags().iter().map(|f| f.to_owned()));
        }

        Ok(tags)
    }

    /// Push a manifest to the registry
    ///
    /// ImageIndex will be pushed with a version tag if version is set
    /// ImageManifest will always be pushed with a digest reference
    #[tracing::instrument(skip_all, fields(otel.name = name, otel.version = version))]
    pub async fn push_manifest(
        &mut self,
        name: &str,
        manifest: Manifest,
        version: Option<&str>,
    ) -> Result<()> {
        let (url, data, content_type) = match manifest {
            Manifest::Index(index) => {
                let version = version.context("`version` required for pushing an ImageIndex")?;
                let url = build_url!(&self.registry, "v2/{}/manifests/{}", name, version);
                let data = serde_json::to_string(&index)?;
                (url, data, "application/vnd.oci.image.index.v1+json")
            }
            Manifest::Manifest(manifest) => {
                let data = serde_json::to_string(&manifest)?;
                let data_digest = digest(&data);
                let url = build_url!(
                    &self.registry,
                    "/v2/{}/manifests/{}",
                    name,
                    data_digest.as_ref()
                );
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
        }
        Ok(())
    }

    /// Pull a manifest from the registry
    ///
    /// If the manifest does not exist, Ok<None> is returned
    /// If any other error happens, an Err is returned
    #[tracing::instrument(skip_all, fields(otel.name = name, otel.reference = reference))]
    pub async fn pull_manifest(&mut self, name: &str, reference: &str) -> Result<Option<Manifest>> {
        let url = build_url!(&self.registry, "/v2/{}/manifests/{}", name, reference);
        let request = self.transport.get(url).header(
            "Accept",
            "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json",
        );
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(None),
            StatusCode::OK => {}
            status => return Err(PyOciError::from((status, response.text().await?)).into()),
        }

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
    pub async fn delete_manifest(&mut self, name: &str, reference: &str) -> Result<()> {
        let url = build_url!(&self.registry, "/v2/{}/manifests/{}", name, reference);
        let request = self.transport.delete(url);
        let response = self.transport.send(request).await?;
        match response.status() {
            StatusCode::ACCEPTED => Ok(()),
            status => Err(PyOciError::from((status, response.text().await?)).into()),
        }
    }
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

    use super::*;

    #[test]
    fn test_build_url() -> Result<()> {
        let url = build_url!(
            &Url::parse("https://example.com").expect("valid url"),
            "/foo/{}/",
            "latest"
        );
        assert_eq!(url.as_str(), "https://example.com/foo/latest/");
        Ok(())
    }

    #[test]
    fn test_build_url_absolute() -> Result<()> {
        let url = build_url!(
            &Url::parse("https://example.com").expect("valid url"),
            "{}/foo?bar=baz&qaz=sha:123",
            "http://pyoci.nl"
        );
        assert_eq!(url.as_str(), "http://pyoci.nl/foo?bar=baz&qaz=sha:123");
        Ok(())
    }

    #[test]
    fn test_build_url_double_period() {
        let x = || -> Result<Url> {
            Ok(build_url!(
                &Url::parse("https://example.com").expect("valid url"),
                "/foo/{}/",
                ".."
            ))
        }();
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

        let mut client = Oci::new(Url::parse(&url).expect("valid url"), None).unwrap();
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

        let mut client = Oci::new(Url::parse(&url).expect("valid url"), None).unwrap();
        let blob = Blob::new("hello".into(), "application/octet-stream");
        let _ = client.push_blob("mockserver/foobar", blob).await;

        for mock in mocks {
            mock.assert_async().await;
        }
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

        let mut pyoci = Oci::new(Url::parse(&url).expect("valid url"), None).unwrap();

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

        let mut pyoci = Oci::new(Url::parse(&url).expect("valid url"), None).unwrap();

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

    #[test]
    fn link() {
        let link = Link::try_from(&HeaderValue::from_static("</v2/allexveldman/hello_world/tags/list?last=0.0.1-example.1.poetry.2824051&n=5>; rel=\"next\"")).unwrap();
        assert_eq!(
            link.0,
            "/v2/allexveldman/hello_world/tags/list?last=0.0.1-example.1.poetry.2824051&n=5"
        )
    }
}
