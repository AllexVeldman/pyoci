use askama::Template;
use axum::{
    debug_handler,
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{header, HeaderMap},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use http::StatusCode;
use tracing::{info_span, Instrument};

use crate::{package, pyoci::PyOciError, templates, PyOci};

#[derive(Debug)]
// Custom error type to translate between anyhow/axum
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self.0.downcast_ref::<PyOciError>() {
            Some(err) => (err.status, err.message.clone()).into_response(),
            None => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self.0)).into_response(),
        }
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
/// Request Router
pub fn router() -> Router {
    // TODO: Validate HOST header against a list of allowed hosts
    Router::new()
        .route(
            "/",
            get(|| async { Redirect::to(env!("CARGO_PKG_HOMEPAGE")) }),
        )
        .route("/:registry/:namespace/:package/", get(list_package))
        .route(
            "/:registry/:namespace/:package/:filename",
            get(download_package),
        )
        .route(
            "/:registry/:namespace/",
            post(publish_package).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .layer(axum::middleware::from_fn(accesslog_middleware))
        .layer(axum::middleware::from_fn(trace_middleware))
}

/// Log incoming requests
async fn accesslog_middleware(
    method: axum::http::Method,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;

    let status: u16 = response.status().into();
    let user_agent = headers
        .get("user-agent")
        .map(|ua| ua.to_str().unwrap_or(""));
    tracing::info!(
        method = method.to_string(),
        status,
        path = uri.path(),
        user_agent,
        "type" = "request"
    );
    response
}

/// Wrap all incoming requests in a fetch trace
async fn trace_middleware(
    method: axum::http::Method,
    uri: axum::http::Uri,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let span = info_span!(
        "fetch",
        otel.path = uri.path(),
        otel.method = method.as_str(),
        otel.span_kind = "server"
    );
    next.run(request).instrument(span).await
}

/// List package request handler
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn list_package(
    headers: HeaderMap,
    path_params: Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let auth = match headers.get("Authorization") {
        Some(auth) => Some(auth.to_str()?.to_owned()),
        None => None,
    };
    let package: package::Info = path_params.0.try_into()?;

    let mut client = PyOci::new(package.registry.clone(), auth)?;
    // Fetch at most 100 package versions
    let files = client.list_package_files(&package, 100).await?;

    // TODO: swap to application/vnd.pypi.simple.v1+json
    let template = templates::ListPackageTemplate { files };
    Ok(Html(template.render().expect("valid template")))
}

/// Download package request handler
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn download_package(
    path_params: Path<(String, String, Option<String>, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let auth = match headers.get("Authorization") {
        Some(auth) => Some(auth.to_str()?.to_owned()),
        None => None,
    };
    let package: package::Info = path_params.0.try_into()?;

    let mut client = PyOci::new(package.registry.clone(), auth)?;
    let data = client
        .download_package_file(&package)
        .await?
        .bytes()
        .await?;

    Ok((
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", package.filename()),
        )],
        data,
    ))
}

/// Publish package request handler
///
/// ref: https://warehouse.pypa.io/api-reference/legacy.html#upload-api
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn publish_package(
    Path((registry, namespace)): Path<(String, String)>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<String, AppError> {
    let form_data = UploadForm::from_multipart(multipart).await?;

    let auth = match headers.get("Authorization") {
        Some(auth) => Some(auth.to_str()?.to_owned()),
        None => None,
    };
    let package: package::Info = (registry, namespace, None, form_data.filename).try_into()?;
    let mut client = PyOci::new(package.registry.clone(), auth)?;

    client
        .publish_package_file(&package, form_data.content)
        .await?;
    Ok("Published".into())
}

/// Form data for the upload API
///
/// ref: https://warehouse.pypa.io/api-reference/legacy.html#upload-api
#[derive(Debug)]
struct UploadForm {
    filename: String,
    content: Vec<u8>,
}

impl UploadForm {
    /// Convert a Multipart into an UploadForm
    ///
    /// Returns MultiPartError if the form can't be parsed
    async fn from_multipart(mut multipart: Multipart) -> anyhow::Result<Self> {
        let mut action = None;
        let mut protocol_version = None;
        let mut content = None;
        let mut filename = None;
        while let Some(field) = multipart.next_field().await? {
            match field.name() {
                Some(":action") => action = Some(field.text().await?),
                Some("protocol_version") => protocol_version = Some(field.text().await?),
                Some("content") => {
                    filename = field.file_name().map(|s| s.to_string());
                    content = Some(field.bytes().await?)
                }
                _ => (),
            }
        }

        match action {
            Some(action) if action == "file_upload" => (),
            None => {
                return Err(PyOciError::from((
                    StatusCode::BAD_REQUEST,
                    "Missing ':action' form-field",
                ))
                .into())
            }
            _ => {
                return Err(PyOciError::from((
                    StatusCode::BAD_REQUEST,
                    "Invalid ':action' form-field",
                ))
                .into())
            }
        };

        match protocol_version {
            Some(protocol_version) if protocol_version == "1" => (),
            None => {
                return Err(PyOciError::from((
                    StatusCode::BAD_REQUEST,
                    "Missing 'protocol_version' form-field",
                ))
                .into())
            }
            _ => {
                return Err(PyOciError::from((
                    StatusCode::BAD_REQUEST,
                    "Invalid 'protocol_version' form-field",
                ))
                .into())
            }
        };

        let content = match content {
            None => {
                return Err(PyOciError::from((
                    StatusCode::BAD_REQUEST,
                    "Missing 'content' form-field",
                ))
                .into())
            }
            Some(content) if content.is_empty() => {
                return Err(
                    PyOciError::from((StatusCode::BAD_REQUEST, "No 'content' provided")).into(),
                )
            }
            Some(content) => content,
        };

        let filename = match filename {
            Some(filename) if filename.is_empty() => {
                return Err(
                    PyOciError::from((StatusCode::BAD_REQUEST, "No 'filename' provided")).into(),
                )
            }
            Some(filename) => filename,
            None => {
                return Err(PyOciError::from((
                    StatusCode::BAD_REQUEST,
                    "'content' form-field is missing a 'filename'",
                ))
                .into())
            }
        };

        Ok(Self {
            filename,
            content: content.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use bytes::Bytes;
    use indoc::formatdoc;
    use oci_spec::{
        distribution::{TagList, TagListBuilder},
        image::{
            Arch, DescriptorBuilder, ImageIndex, ImageIndexBuilder, ImageManifest,
            ImageManifestBuilder, Os, PlatformBuilder,
        },
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn publish_package_missing_action() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\"submit-name\"\r\n\
            \r\n\
            Larry\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "Missing ':action' form-field");
    }

    #[tokio::test]
    async fn publish_package_invalid_action() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            not-file_download\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "Invalid ':action' form-field");
    }

    #[tokio::test]
    async fn publish_package_missing_protocol_version() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "Missing 'protocol_version' form-field");
    }

    #[tokio::test]
    async fn publish_package_invalid_protocol_version() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            2\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "Invalid 'protocol_version' form-field");
    }

    #[tokio::test]
    async fn publish_package_missing_content() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "Missing 'content' form-field");
    }

    #[tokio::test]
    async fn publish_package_empty_content() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"\r\n\
            \r\n\
            \r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "No 'content' provided");
    }

    #[tokio::test]
    async fn publish_package_content_missing_filename() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"\r\n\
            \r\n\
            someawesomepackagedata\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "'content' form-field is missing a 'filename'");
    }

    #[tokio::test]
    async fn publish_package_content_filename_empty() {
        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"; filename=\"\"\r\n\
            \r\n\
            someawesomepackagedata\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "No 'filename' provided");
    }

    #[tokio::test]
    async fn publish_package_url_encoded_registry() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let mocks = vec![
            // Mock the server, in order of expected requests
            // IndexManifest does not yet exist
            server
                .mock("GET", "/v2/mockserver/foobar/manifests/1.0.0")
                .with_status(404)
                .create_async()
                .await,
            // HEAD request to check if blob exists for:
            // - layer
            // - config
            server
                .mock(
                    "HEAD",
                    mockito::Matcher::Regex(r"/v2/mockserver/foobar/blobs/.+".to_string()),
                )
                .expect(2)
                .with_status(404)
                .create_async()
                .await,
            // POST request with blob for layer
            server
                .mock("POST", "/v2/mockserver/foobar/blobs/uploads/")
                .with_status(202) // ACCEPTED
                .with_header(
                    "Location",
                    &format!("{url}/v2/mockserver/foobar/blobs/uploads/1?_state=uploading"),
                )
                .create_async()
                .await,
            server
                .mock("PUT", "/v2/mockserver/foobar/blobs/uploads/1?_state=uploading&digest=sha256%3Ab7513fb69106a855b69153582dec476677b3c79f4a13cfee6fb7a356cfa754c0")
                .with_status(201) // CREATED
                .create_async()
                .await,
            // POST request with blob for config
            server
                .mock("POST", "/v2/mockserver/foobar/blobs/uploads/")
                .with_status(202) // ACCEPTED
                .with_header(
                    "Location",
                    &format!("{url}/v2/mockserver/foobar/blobs/uploads/2?_state=uploading"),
                )
                .create_async()
                .await,
            server
                .mock("PUT", "/v2/mockserver/foobar/blobs/uploads/2?_state=uploading&digest=sha256%3A44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a")
                .with_status(201) // CREATED
                .create_async()
                .await,
            // PUT request to create Manifest
            server
                .mock("PUT", "/v2/mockserver/foobar/manifests/sha256:7ffd96d9eab411893eeacfa906e30956290a07b0141d7c1dd54c9fd5c7c48cf5")
                .match_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
                .match_body(r#"{"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json","artifactType":"application/pyoci.package.v1","config":{"mediaType":"application/vnd.oci.empty.v1+json","digest":"sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a","size":2},"layers":[{"mediaType":"application/pyoci.package.v1","digest":"sha256:b7513fb69106a855b69153582dec476677b3c79f4a13cfee6fb7a356cfa754c0","size":22}]}"#)
                .with_status(201) // CREATED
                .create_async()
                .await,
            // PUT request to create Index
            server
                .mock("PUT", "/v2/mockserver/foobar/manifests/1.0.0")
                .match_header("Content-Type", "application/vnd.oci.image.index.v1+json")
                .match_body(r#"{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","artifactType":"application/pyoci.package.v1","manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:7ffd96d9eab411893eeacfa906e30956290a07b0141d7c1dd54c9fd5c7c48cf5","size":422,"platform":{"architecture":".tar.gz","os":"any"}}]}"#)
                .with_status(201) // CREATED
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"; filename=\"foobar-1.0.0.tar.gz\"\r\n\
            \r\n\
            someawesomepackagedata\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri(format!("/{encoded_url}/mockserver/"))
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(&body, "Published");
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn list_package() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let tags_list = TagListBuilder::default()
            .name("test-package")
            .tags(vec!["0.1.0".to_string(), "1.2.3".to_string()])
            .build()
            .unwrap();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest("FooBar")
                .size(6)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let index_123 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest("FooBar")
                .size(6)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let mocks = vec![
            // List tags
            server
                .mock("GET", "/v2/mockserver/test_package/tags/list")
                .with_status(200)
                .with_body(serde_json::to_string::<TagList>(&tags_list).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0 manifest
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/0.1.0")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.index.v1+json")
                .with_body(serde_json::to_string::<ImageIndex>(&index_010).unwrap())
                .create_async()
                .await,
            // Pull 1.2.3 manifest
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/1.2.3")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.index.v1+json")
                .with_body(serde_json::to_string::<ImageIndex>(&index_123).unwrap())
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();
        let req = Request::builder()
            .method("GET")
            .uri(format!("/{encoded_url}/mockserver/test-package/"))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            formatdoc!(
                r#"
                <!DOCTYPE html>
                <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <title>PyOCI</title>
                </head>
                <body>
                    <a href="/{encoded_url}/mockserver/test_package/test_package-1.2.3.tar.gz">test_package-1.2.3.tar.gz</a>
                    <a href="/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz">test_package-0.1.0.tar.gz</a>
                </body>
                </html>"#
            )
        );
    }

    #[tokio::test]
    async fn list_package_missing_manifest() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let tags_list = TagListBuilder::default()
            .name("test-package")
            .tags(vec!["0.1.0".to_string(), "1.2.3".to_string()])
            .build()
            .unwrap();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest("FooBar")
                .size(6)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let mocks = vec![
            // List tags
            server
                .mock("GET", "/v2/mockserver/test_package/tags/list")
                .with_status(200)
                .with_body(serde_json::to_string::<TagList>(&tags_list).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0 manifest
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/0.1.0")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.index.v1+json")
                .with_body(serde_json::to_string::<ImageIndex>(&index_010).unwrap())
                .create_async()
                .await,
            // Pull 1.2.3 manifest
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/1.2.3")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(404)
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();
        let req = Request::builder()
            .method("GET")
            .uri(format!("/{encoded_url}/mockserver/test-package/"))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "ImageIndex does not exist");
    }

    #[tokio::test]
    async fn download_package() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let index = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest("FooBar")
                    .size(6)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".whl".to_string()))
                            .os(Os::Other("any".to_string()))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest("sha256:manifest-digest")
                    .size(6)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".tar.gz".to_string()))
                            .os(Os::Other("any".to_string()))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();

        let manifest = ImageManifestBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.manifest.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .config(
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.empty.v1+json")
                    .digest("sha256:config-digest")
                    .size(0)
                    .build()
                    .unwrap(),
            )
            .layers(vec![DescriptorBuilder::default()
                .media_type("application/pyoci.package.v1")
                .digest("sha256:layer-digest")
                .size(42)
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let blob = Bytes::from(vec![1, 2, 3]);

        let mocks = vec![
            // Pull 0.1.0 index
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/0.1.0")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.index.v1+json")
                .with_body(serde_json::to_string::<ImageIndex>(&index).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0.tar.gz manifest
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/sha256:manifest-digest")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.manifest.v1+json")
                .with_body(serde_json::to_string::<ImageManifest>(&manifest).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0.tar.gz blob
            server
                .mock("GET", "/v2/mockserver/test_package/blobs/sha256:layer-digest")
                .with_status(200)
                .with_body(blob.clone())
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, blob);
    }

    #[tokio::test]
    async fn download_package_missing_manifest() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let index = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest("FooBar")
                    .size(6)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".whl".to_string()))
                            .os(Os::Other("any".to_string()))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest("sha256:manifest-digest")
                    .size(6)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".tar.gz".to_string()))
                            .os(Os::Other("any".to_string()))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();

        let mocks = vec![
            // Pull 0.1.0 index
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/0.1.0")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.index.v1+json")
                .with_body(serde_json::to_string::<ImageIndex>(&index).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0.tar.gz manifest
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/sha256:manifest-digest")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(404)
                .create_async()
                .await,

            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "ImageManifest does not exist");
    }

    #[tokio::test]
    async fn download_package_missing_architecture() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let index = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest("FooBar")
                .size(6)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".whl".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let mocks = vec![
            // Pull 0.1.0 index
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/0.1.0")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(200)
                .with_header("content-type", "application/vnd.oci.image.index.v1+json")
                .with_body(serde_json::to_string::<ImageIndex>(&index).unwrap())
                .create_async()
                .await,

            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "Requested architecture '.tar.gz' not available");
    }

    #[tokio::test]
    async fn download_package_missing_index() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let mocks = vec![
            // Pull 0.1.0 index
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/0.1.0")
                .match_header(
                    "accept",
                    "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json")
                .with_status(404)
                .create_async()
                .await,

            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let router = router();
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "ImageIndex does not exist");
    }
}
