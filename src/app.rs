use std::{
    collections::{BTreeSet, HashMap},
    convert::Infallible,
};

use axum::{
    debug_handler,
    extract::{multipart::MultipartError, DefaultBodyLimit, Multipart, Path, Request, State},
    http::{header, HeaderMap},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::{rejection::HostRejection, Host};
use bytes::Bytes;
use handlebars::Handlebars;
use http::{header::CACHE_CONTROL, HeaderValue, StatusCode};
use serde::{ser::SerializeMap, Serialize, Serializer};
use tower::Service;
use tracing::{debug, info_span, Instrument};

use crate::{
    error::PyOciError,
    middleware::EncodeNamespace,
    package::{Package, WithFileName},
    Env, PyOci,
};

#[derive(Debug)]
// Custom error type to translate between anyhow/axum
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let any_err = match self.0.downcast::<PyOciError>() {
            Ok(err) => return err.into_response(),
            Err(err) => err,
        };
        let any_err = match any_err.downcast::<MultipartError>() {
            Ok(err) => return err.into_response(),
            Err(err) => err,
        };
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{any_err:#}")).into_response()
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

#[derive(Debug, Clone)]
struct PyOciState<'a> {
    /// Subpath `PyOCI` is hosted on
    subpath: Option<String>,
    /// Maximum versions `PyOCI` will fetch when listing a package
    max_versions: usize,
    /// HTML Template registry
    templates: Handlebars<'a>,
}

// The PyOCI Service
pub fn pyoci_service(
    env: &Env,
) -> impl Service<Request, Response = Response, Error = Infallible, Future: Send> + '_ + Clone {
    EncodeNamespace::new(router(env), env.path.as_deref())
}

/// Request Router
fn router(env: &Env) -> Router {
    let pyoci_routes = Router::new()
        .fallback(
            get(|| async { StatusCode::NOT_FOUND })
                .layer(axum::middleware::from_fn(cache_control_middleware)),
        )
        .route(
            "/",
            get(|| async { Redirect::to(env!("CARGO_PKG_HOMEPAGE")) })
                .layer(axum::middleware::from_fn(cache_control_middleware)),
        )
        .route("/{registry}/{namespace}/{package}/", get(list_package))
        .route(
            "/{registry}/{namespace}/{package}/json",
            get(list_package_json),
        )
        .route(
            "/{registry}/{namespace}/{package}/{filename}",
            get(download_package).delete(delete_package_version),
        )
        .route(
            "/{registry}/{namespace}/",
            post(publish_package).layer(DefaultBodyLimit::max(env.body_limit)),
        );
    let router = match env.path {
        Some(ref subpath) => Router::new().nest(subpath, pyoci_routes),
        _ => pyoci_routes,
    };

    // Setup templates
    let mut template_reg = Handlebars::new();
    template_reg.set_strict_mode(true);

    #[cfg(debug_assertions)]
    template_reg.set_dev_mode(true);

    template_reg
        .register_template_file("html_list_pkg", "./templates/list-package.html")
        .expect("Invalid template");

    router
        .layer(axum::middleware::from_fn(accesslog_middleware))
        .layer(axum::middleware::from_fn(trace_middleware))
        .route("/health", get(|| async { StatusCode::OK }))
        .with_state(PyOciState {
            subpath: env.path.clone(),
            max_versions: env.max_versions,
            templates: template_reg,
        })
}

/// Add cache-control for unmatched routes
///
/// This allows downstream caches to not wake up the server for unmatched paths
/// like scrapers and vulnerability scanners
async fn cache_control_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        // Cache the response for 7 days
        HeaderValue::from_str("max-age=604800, public").unwrap(),
    );
    response
}

/// Log incoming requests
async fn accesslog_middleware(
    method: axum::http::Method,
    host: Result<Host, HostRejection>,
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

    tracing::debug!("Accept: {:?}", headers);
    tracing::info!(
        host = host.map(|value| value.0).unwrap_or_default(),
        "type" = "request",
        status,
        method = method.to_string(),
        path = uri.path(),
        user_agent,
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

#[derive(serde::Serialize)]
struct ListPkgTemplateData<'a> {
    files: Vec<Package<'a, WithFileName>>,
    subpath: Option<String>,
}

/// List package request handler
///
/// (registry, namespace, package)
#[tracing::instrument(skip_all)]
async fn list_package(
    State(PyOciState {
        subpath,
        max_versions,
        templates,
    }): State<PyOciState<'_>>,
    headers: HeaderMap,
    Path((registry, namespace, package_name)): Path<(String, String, String)>,
) -> Result<Html<String>, AppError> {
    let package = Package::new(&registry, &namespace, &package_name);

    let mut client = PyOci::new(package.registry()?, get_auth(&headers));
    // Fetch at most 100 package versions
    let files = client.list_package_files(&package, max_versions).await?;

    let data = ListPkgTemplateData { files, subpath };

    Ok(Html(templates.render("html_list_pkg", &data)?))
}

/// JSON response for listing a package
#[derive(Serialize)]
struct ListJson {
    info: Info,
    #[serde(serialize_with = "ser_releases")]
    releases: BTreeSet<String>,
}

/// Serializer for the releases field
///
/// The releases serialize to {"<version>":[]} with a key for every version.
/// The list is kept empty so we don't need to query for each version manifest
fn ser_releases<S>(releases: &BTreeSet<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = serializer.serialize_map(Some(releases.len()))?;
    for version in releases {
        map.serialize_entry::<String, [()]>(version, &[])?;
    }
    map.end()
}

#[derive(Serialize)]
struct Info {
    name: String,
    project_urls: HashMap<String, String>,
}

/// List package JSON request handler
///
/// Allows listing all releases without the additional file information
/// Specifically this is used by Renovate to determine the available releases
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn list_package_json(
    headers: HeaderMap,
    Path((registry, namespace, package_name)): Path<(String, String, String)>,
) -> Result<Json<ListJson>, AppError> {
    let package = Package::new(&registry, &namespace, &package_name);

    let mut client = PyOci::new(package.registry()?, get_auth(&headers));
    let versions = client.list_package_versions(&package).await?;

    let mut project_urls = HashMap::new();
    if let Some(last_version) = versions.last() {
        if let Some(package) = client
            .package_info_for_ref(&package, last_version)
            .await?
            .first()
            .map(Package::project_urls)
            .unwrap()
        {
            project_urls = package;
        }
    }
    let response = ListJson {
        info: Info {
            name: package.name().to_string(),
            project_urls,
        },
        releases: versions,
    };

    Ok(Json(response))
}

/// Download package request handler
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn download_package(
    Path((registry, namespace, _distribution, filename)): Path<(String, String, String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let package = Package::from_filename(&registry, &namespace, &filename)?;

    let mut client = PyOci::new(package.registry()?, get_auth(&headers));
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

/// Delete package version request handler
///
/// This endpoint does not exist as an official spec in the python ecosystem
/// and the underlying OCI distribution spec is not supported by default for some registries
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn delete_package_version(
    Path((registry, namespace, name, version)): Path<(String, String, String, String)>,
    headers: HeaderMap,
) -> Result<String, AppError> {
    let package = Package::new(&registry, &namespace, &name).with_oci_file(&version, "");

    let mut client = PyOci::new(package.registry()?, get_auth(&headers));
    client.delete_package_version(&package).await?;
    Ok("Deleted".into())
}

/// Publish package request handler
///
/// ref: <https://warehouse.pypa.io/api-reference/legacy.html#upload-api>
#[debug_handler]
#[tracing::instrument(skip_all)]
async fn publish_package(
    Path((registry, namespace)): Path<(String, String)>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<String, AppError> {
    let form_data = UploadForm::from_multipart(multipart).await?;

    let package = Package::from_filename(&registry, &namespace, &form_data.filename)?;
    let mut client = PyOci::new(package.registry()?, get_auth(&headers));

    client
        .publish_package_file(
            &package,
            form_data.content,
            form_data.labels,
            form_data.sha256,
            form_data.project_urls,
        )
        .await?;
    Ok("Published".into())
}

/// Parse the Authentication header, if provided
fn get_auth(headers: &HeaderMap) -> Option<HeaderValue> {
    let auth = headers.get("Authorization").map(|auth| {
        let mut auth = auth.to_owned();
        auth.set_sensitive(true);
        auth
    });
    if auth.is_none() {
        tracing::warn!("No Authorization header provided");
    }
    auth
}

/// Form data for the upload API
///
/// ref: <https://docs.pypi.org/api/upload/>
#[derive(Debug, Eq, PartialEq)]
struct UploadForm {
    filename: String,
    content: Vec<u8>,
    labels: HashMap<String, String>,
    sha256: Option<String>,
    project_urls: HashMap<String, String>,
}

impl UploadForm {
    /// Convert a Multipart into an `UploadForm`
    ///
    /// Returns `MultiPartError` if the form can't be parsed
    async fn from_multipart(mut multipart: Multipart) -> anyhow::Result<Self> {
        let mut action = None;
        let mut protocol_version = None;
        let mut content = None;
        let mut filename = None;
        let mut sha256 = None;
        let mut labels = HashMap::new();
        let mut project_urls = HashMap::new();

        // Extract the fields from the form
        while let Some(field) = multipart.next_field().await? {
            let Some(field_name) = field.name().map(ToOwned::to_owned) else {
                continue;
            };

            match field_name.as_str() {
                ":action" => action = Some(field.text().await?),
                "protocol_version" => protocol_version = Some(field.text().await?),
                "content" => {
                    filename = field.file_name().map(ToString::to_string);
                    content = Some(field.bytes().await?);
                }
                "classifiers" => {
                    let classifier = field.text().await?;
                    Self::parse_classifier(&classifier, &mut labels);
                }
                "project_urls" => {
                    let project_url = field.text().await?;
                    Self::parse_project_url(&project_url, &mut project_urls);
                }
                "sha256_digest" => sha256 = Some(field.text().await?),
                name => debug!("Discarding field '{name}': {}", field.text().await?),
            }
        }
        Self::validate_action(action.as_deref())?;
        Self::validate_protocol(protocol_version.as_deref())?;
        let content = Self::unwrap_content(content)?;
        let filename = Self::unwrap_filename(filename)?;

        Ok(Self {
            filename,
            content: content.into(),
            labels,
            sha256,
            project_urls,
        })
    }

    #[allow(clippy::doc_markdown)]
    /// Parse a classifier and insert it into the labels map
    ///
    /// Classifier format:
    /// `"PyOCI :: Label :: <Key> :: <Value>"`
    ///
    /// Any other format will be discarded
    fn parse_classifier(classifier: &str, labels: &mut HashMap<String, String>) {
        if let Some(label) = classifier.strip_prefix("PyOCI :: Label :: ") {
            if let [key, value] = label.splitn(2, " :: ").collect::<Vec<_>>()[..] {
                labels.insert(key.to_string(), value.to_string());
                debug!("Found label '{key}={value}'");
            } else {
                debug!("Invalid PyOci label '{label}'");
            }
        } else {
            debug!("Discarding field 'classifiers': {classifier}");
        }
    }

    /// Parse a project URL and insert it into the project URLs map
    ///
    /// Project URL format:
    /// `"<key>, <URL>"`
    fn parse_project_url(project_url: &str, project_urls: &mut HashMap<String, String>) {
        if let [key, value] = project_url.splitn(2, ", ").collect::<Vec<_>>()[..] {
            project_urls.insert(key.to_string(), value.to_string());
            debug!("Found Project-URL '{key}={value}'");
        } else {
            debug!("Invalid Project-URL '{project_url}'");
        }
    }

    /// Validate the ":action" is "`file_upload`"
    fn validate_action(action: Option<&str>) -> Result<(), PyOciError> {
        match action {
            Some("file_upload") => Ok(()),
            None => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "Missing ':action' form-field",
            ))),
            _ => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "Invalid ':action' form-field",
            ))),
        }
    }

    // Validate the protocol version is "1"
    fn validate_protocol(protocol_version: Option<&str>) -> Result<(), PyOciError> {
        match protocol_version {
            Some("1") => Ok(()),
            None => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "Missing 'protocol_version' form-field",
            ))),
            _ => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "Invalid 'protocol_version' form-field",
            ))),
        }
    }

    fn unwrap_content(content: Option<Bytes>) -> Result<Bytes, PyOciError> {
        match content {
            None => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "Missing 'content' form-field",
            ))),
            Some(content) if content.is_empty() => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "No 'content' provided",
            ))),
            Some(content) => Ok(content),
        }
    }

    fn unwrap_filename(filename: Option<String>) -> Result<String, PyOciError> {
        match filename {
            None => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "'content' form-field is missing a 'filename'",
            ))),
            Some(filename) if filename.is_empty() => Err(PyOciError::from((
                StatusCode::BAD_REQUEST,
                "No 'filename' provided",
            ))),
            Some(filename) => Ok(filename),
        }
    }
}

#[allow(clippy::doc_markdown, clippy::too_many_lines)]
#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use super::*;
    use crate::{clean_subpath, oci::digest};

    use axum::{
        body::{to_bytes, Body},
        extract::{FromRequest, Request},
    };
    use bytes::Bytes;
    use http::HeaderValue;
    use indoc::formatdoc;
    use oci_spec::{
        distribution::{TagList, TagListBuilder},
        image::{
            Arch, DescriptorBuilder, ImageIndex, ImageIndexBuilder, ImageManifest,
            ImageManifestBuilder, Os, PlatformBuilder,
        },
    };
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    #[test]
    fn test_get_auth() {
        let mut headers = HeaderMap::new();
        headers.append("Authorization", "foo".try_into().unwrap());
        let auth = get_auth(&headers);
        assert_eq!(auth, Some(HeaderValue::try_from("foo").unwrap()));
        assert!(auth.unwrap().is_sensitive());
    }

    #[test]
    fn test_get_auth_none() {
        let headers = HeaderMap::new();
        let auth = get_auth(&headers);
        assert_eq!(auth, None);
    }

    #[tokio::test]
    async fn upload_form_missing_action() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\"submit-name\"\r\n\
            \r\n\
            Larry\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "Missing ':action' form-field");
    }

    #[tokio::test]
    async fn upload_form_invalid_action() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            not-file_download\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "Invalid ':action' form-field");
    }

    #[tokio::test]
    async fn upload_form_missing_protocol_version() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "Missing 'protocol_version' form-field");
    }

    #[tokio::test]
    async fn upload_form_invalid_protocol_version() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            2\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "Invalid 'protocol_version' form-field");
    }

    #[tokio::test]
    async fn upload_form_missing_content() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "Missing 'content' form-field");
    }

    #[tokio::test]
    async fn upload_form_empty_content() {
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
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "No 'content' provided");
    }

    #[tokio::test]
    async fn upload_form_content_missing_filename() {
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
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            result.message,
            "'content' form-field is missing a 'filename'"
        );
    }

    #[tokio::test]
    async fn upload_form_content_filename_empty() {
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
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect_err("Expected Error")
            .downcast::<PyOciError>()
            .expect("Expected PyOciError");
        assert_eq!(result.status, StatusCode::BAD_REQUEST);
        assert_eq!(result.message, "No 'filename' provided");
    }

    #[tokio::test]
    /// Minimal valid form
    async fn upload_form() {
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
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect("Valid Form");
        assert_eq!(result.filename, "foobar-1.0.0.tar.gz");
        assert_eq!(
            result.content,
            String::from("someawesomepackagedata").into_bytes()
        );
        assert_eq!(result.labels, HashMap::new());
        assert_eq!(result.sha256, None);
    }

    #[tokio::test]
    /// Check if we can extract "PyOci :: Label :: " classifiers
    async fn upload_form_labels() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"classifiers\"\r\n\
            \r\n\
            Programming Language :: Python :: 3.13\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"classifiers\"\r\n\
            \r\n\
            PyOCI :: Label :: org.opencontainers.image.url :: https://github.com/allexveldman/pyoci\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"classifiers\"\r\n\
            \r\n\
            PyOCI :: Label :: other-label :: foobar\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"; filename=\"foobar-1.0.0.tar.gz\"\r\n\
            \r\n\
            someawesomepackagedata\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect("Valid Form");
        assert_eq!(
            result.labels,
            HashMap::from([
                (
                    "org.opencontainers.image.url".to_string(),
                    "https://github.com/allexveldman/pyoci".to_string()
                ),
                ("other-label".to_string(), "foobar".to_string())
            ])
        );
    }

    #[tokio::test]
    /// Check if project URLs are properly parsed
    async fn upload_form_project_urls() {
        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"project_urls\"\r\n\
            \r\n\
            Repository, https://github/allexveldman/pyoci\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"project_urls\"\r\n\
            \r\n\
            Homepage, https://pyoci.com\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"; filename=\"foobar-1.0.0.tar.gz\"\r\n\
            \r\n\
            someawesomepackagedata\r\n\
            --foobar--\r\n";
        let req: Request<Body> = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.to_string().into())
            .unwrap();
        let multipart = Multipart::from_request(req, &()).await.unwrap();

        let result = UploadForm::from_multipart(multipart)
            .await
            .expect("Valid Form");
        assert_eq!(
            result,
            UploadForm {
                filename: "foobar-1.0.0.tar.gz".to_string(),
                content: String::from("someawesomepackagedata").into_bytes(),
                labels: HashMap::new(),
                sha256: None,
                project_urls: HashMap::from([
                    (
                        "Repository".to_string(),
                        "https://github/allexveldman/pyoci".to_string()
                    ),
                    ("Homepage".to_string(), "https://pyoci.com".to_string())
                ])
            }
        );
    }

    #[tokio::test]
    async fn cache_control_unmatched() {
        let router = router(&Env::default());

        let req = Request::builder()
            .method("GET")
            .uri("/foo")
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get("Cache-Control"),
            Some(&HeaderValue::from_str("max-age=604800, public").unwrap())
        );
    }

    #[tokio::test]
    async fn cache_control_root() {
        let router = router(&Env::default());

        let req = Request::builder()
            .method("GET")
            .uri("/")
            .body(Body::empty())
            .unwrap();
        let response = router.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("Cache-Control"),
            Some(&HeaderValue::from_str("max-age=604800, public").unwrap())
        );
    }

    #[tokio::test]
    async fn publish_package_body_limit() {
        let env = Env {
            body_limit: 10,
            ..Env::default()
        };
        let service = pyoci_service(&env);

        let form = "Exceeds max body limit";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.into())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn publish_package_content_filename_invalid() {
        let env = Env::default();
        let service = pyoci_service(&env);

        let form = "--foobar\r\n\
            Content-Disposition: form-data; name=\":action\"\r\n\
            \r\n\
            file_upload\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"protocol_version\"\r\n\
            \r\n\
            1\r\n\
            --foobar\r\n\
            Content-Disposition: form-data; name=\"content\"; filename=\".env\"\r\n\
            \r\n\
            someawesomepackagedata\r\n\
            --foobar--\r\n";
        let req = Request::builder()
            .method("POST")
            .uri("/pypi/pytest/")
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.into())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();
        assert_eq!(&body, "Unkown filetype '.env'");
    }

    #[tokio::test]
    async fn publish_package() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        // Set timestamp to fixed time
        crate::time::set_timestamp(1_732_134_216);

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
                .mock("PUT", "/v2/mockserver/foobar/manifests/sha256:e281659053054737342fd0c74a7605c4678c227db1e073260b44f845dfdf535a")
                .match_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
                .with_status(201) // CREATED
                .create_async()
                .await,
            // PUT request to create Index
            server
                .mock("PUT", "/v2/mockserver/foobar/manifests/1.0.0")
                .match_header("Content-Type", "application/vnd.oci.image.index.v1+json")
                .with_status(201) // CREATED
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let env = Env::default();
        let service = pyoci_service(&env);

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
            .body(form.into())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
    async fn publish_package_subpath() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        // Set timestamp to fixed time
        crate::time::set_timestamp(1_732_134_216);

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
                .mock("PUT", "/v2/mockserver/foobar/manifests/sha256:e281659053054737342fd0c74a7605c4678c227db1e073260b44f845dfdf535a")
                .match_header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
                .with_status(201) // CREATED
                .create_async()
                .await,
            // PUT request to create Index
            server
                .mock("PUT", "/v2/mockserver/foobar/manifests/1.0.0")
                .match_header("Content-Type", "application/vnd.oci.image.index.v1+json")
                .with_status(201) // CREATED
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let env = Env {
            path: Some("/foo".to_string()),
            ..Env::default()
        };
        let service = pyoci_service(&env);

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
            .uri(format!("/foo/{encoded_url}/mockserver/"))
            .header("Content-Type", "multipart/form-data; boundary=foobar")
            .body(form.into())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
            .tags(vec![
                "0.1.0".to_string(),
                // max_versions is set to 2, so this version will be excluded
                "0.0.1".to_string(),
                "1.2.3".to_string(),
            ])
            .build()
            .unwrap();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest(digest("FooBar"))
                .size(6_u64)
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
                .digest(digest("FooBar"))
                .size(6_u64)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .annotations(HashMap::from([(
                    "com.pyoci.sha256_digest".to_string(),
                    "1234".to_string(),
                )]))
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

        let env = Env {
            max_versions: 2,
            ..Env::default()
        };
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!("/{encoded_url}/mockserver/test-package/"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
                    <a href="/{encoded_url}/mockserver/test_package/test_package-1.2.3.tar.gz#sha256=1234">test_package-1.2.3.tar.gz</a>
                    <a href="/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz">test_package-0.1.0.tar.gz</a>
                </body>
                </html>
                "#
            )
        );
    }

    #[tokio::test]
    async fn list_package_subpath() {
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
                .digest(digest("FooBar"))
                .size(6_u64)
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
                .digest(digest("FooBar"))
                .size(6_u64)
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

        let env = Env {
            path: Some("/foo".to_string()),
            ..Env::default()
        };
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!("/foo/{encoded_url}/mockserver/test-package/"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
                    <a href="/foo/{encoded_url}/mockserver/test_package/test_package-1.2.3.tar.gz">test_package-1.2.3.tar.gz</a>
                    <a href="/foo/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz">test_package-0.1.0.tar.gz</a>
                </body>
                </html>
                "#
            )
        );
    }

    #[tokio::test]
    async fn list_package_multipart_namespace() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let tags_list = TagListBuilder::default()
            .name("test-package")
            .tags(vec![
                "0.1.0".to_string(),
                // max_versions is set to 2, so this version will be excluded
                "0.0.1".to_string(),
                "1.2.3".to_string(),
            ])
            .build()
            .unwrap();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest(digest("FooBar"))
                .size(6_u64)
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
                .digest(digest("FooBar"))
                .size(6_u64)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .annotations(HashMap::from([(
                    "com.pyoci.sha256_digest".to_string(),
                    "1234".to_string(),
                )]))
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let mocks = vec![
            // List tags
            server
                .mock("GET", "/v2/mockserver/subnamespace/test_package/tags/list")
                .with_status(200)
                .with_body(serde_json::to_string::<TagList>(&tags_list).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0 manifest
            server
                .mock("GET", "/v2/mockserver/subnamespace/test_package/manifests/0.1.0")
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
                .mock("GET", "/v2/mockserver/subnamespace/test_package/manifests/1.2.3")
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

        let env = Env {
            max_versions: 2,
            ..Env::default()
        };
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "/{encoded_url}/mockserver/subnamespace/test-package/"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
                    <a href="/{encoded_url}/mockserver/subnamespace/test_package/test_package-1.2.3.tar.gz#sha256=1234">test_package-1.2.3.tar.gz</a>
                    <a href="/{encoded_url}/mockserver/subnamespace/test_package/test_package-0.1.0.tar.gz">test_package-0.1.0.tar.gz</a>
                </body>
                </html>
                "#
            )
        );
    }

    #[tokio::test]
    async fn list_package_multipart_namespace_with_subpath() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let tags_list = TagListBuilder::default()
            .name("test-package")
            .tags(vec![
                "0.1.0".to_string(),
                // max_versions is set to 2, so this version will be excluded
                "0.0.1".to_string(),
                "1.2.3".to_string(),
            ])
            .build()
            .unwrap();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest(digest("FooBar"))
                .size(6_u64)
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
                .digest(digest("FooBar"))
                .size(6_u64)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .annotations(HashMap::from([(
                    "com.pyoci.sha256_digest".to_string(),
                    "1234".to_string(),
                )]))
                .build()
                .unwrap()])
            .build()
            .unwrap();

        let mocks = vec![
            // List tags
            server
                .mock("GET", "/v2/mockserver/subnamespace/test_package/tags/list")
                .with_status(200)
                .with_body(serde_json::to_string::<TagList>(&tags_list).unwrap())
                .create_async()
                .await,
            // Pull 0.1.0 manifest
            server
                .mock("GET", "/v2/mockserver/subnamespace/test_package/manifests/0.1.0")
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
                .mock("GET", "/v2/mockserver/subnamespace/test_package/manifests/1.2.3")
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

        let env = Env {
            max_versions: 2,
            path: Some("/foo".to_string()),
            ..Env::default()
        };
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "/foo/{encoded_url}/mockserver/subnamespace/test-package/"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
                    <a href="/foo/{encoded_url}/mockserver/subnamespace/test_package/test_package-1.2.3.tar.gz#sha256=1234">test_package-1.2.3.tar.gz</a>
                    <a href="/foo/{encoded_url}/mockserver/subnamespace/test_package/test_package-0.1.0.tar.gz">test_package-0.1.0.tar.gz</a>
                </body>
                </html>
                "#
            )
        );
    }

    #[tokio::test]
    async fn list_package_missing_index() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let mocks = vec![
            // List tags
            server
                .mock("GET", "/v2/mockserver/test_package/tags/list")
                .with_status(404)
                .with_body("Server missing message")
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!("/{encoded_url}/mockserver/test-package/"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
        assert_eq!(body, "Server missing message");
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
                .digest(digest("FooBar"))
                .size(6_u64)
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

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!("/{encoded_url}/mockserver/test-package/"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
        assert_eq!(body, "ImageManifest '1.2.3' does not exist");
    }

    #[tokio::test]
    async fn list_package_json() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let tags_list = TagListBuilder::default()
            .name("test-package")
            .tags(vec!["0.1.0".to_string(), "1.2.3".to_string()])
            .build()
            .unwrap();

        let index = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![DescriptorBuilder::default()
                .media_type("application/vnd.oci.image.manifest.v1+json")
                .digest(digest("FooBar"))
                .size(6_u64)
                .platform(
                    PlatformBuilder::default()
                        .architecture(Arch::Other(".tar.gz".to_string()))
                        .os(Os::Other("any".to_string()))
                        .build()
                        .unwrap(),
                )
                .annotations(HashMap::from([(
                    "com.pyoci.project_urls".to_string(),
                    r#"{"Repository": "https://github.com/allexveldman/pyoci"}"#.to_string(),
                )]))
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
            // Pull 1.2.3 manifest for project_urls
            server
                .mock("GET", "/v2/mockserver/test_package/manifests/1.2.3")
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

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!("/{encoded_url}/mockserver/test-package/json"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
            r#"{"info":{"name":"test_package","project_urls":{"Repository":"https://github.com/allexveldman/pyoci"}},"releases":{"0.1.0":[],"1.2.3":[]}}"#
        );
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
                    .digest(digest("FooBar"))
                    .size(6_u64)
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
                    .digest(digest("manifest-digest")) // sha256:bc669544845542470042912a0f61b90499ffc2320b45ea66b0be50439c5aab19
                    .size(6_u64)
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
                    .digest(digest("config-digest")) // sha:7b6a7aed8c63f4480a863fa046048c4bfb77d4514212ad646a5fcadcf8f5da47
                    .size(0_u64)
                    .build()
                    .unwrap(),
            )
            .layers(vec![DescriptorBuilder::default()
                .media_type("application/pyoci.package.v1")
                .digest(digest("layer-digest")) // sha:8a576772defc4006637b27e7b0bef2c8bb6f3f7465d27426f1684da58ea9f969
                .size(42_u64)
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
                .mock("GET", "/v2/mockserver/test_package/manifests/sha256:bc669544845542470042912a0f61b90499ffc2320b45ea66b0be50439c5aab19")
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
                .mock("GET", "/v2/mockserver/test_package/blobs/sha256:8a576772defc4006637b27e7b0bef2c8bb6f3f7465d27426f1684da58ea9f969")
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

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, blob);
    }

    #[tokio::test]
    async fn download_package_subpath() {
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
                    .digest(digest("FooBar"))
                    .size(6_u64)
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
                    .digest(digest("manifest-digest")) // sha256:bc669544845542470042912a0f61b90499ffc2320b45ea66b0be50439c5aab19
                    .size(6_u64)
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
                    .digest(digest("config-digest")) // sha:7b6a7aed8c63f4480a863fa046048c4bfb77d4514212ad646a5fcadcf8f5da47
                    .size(0_u64)
                    .build()
                    .unwrap(),
            )
            .layers(vec![DescriptorBuilder::default()
                .media_type("application/pyoci.package.v1")
                .digest(digest("layer-digest")) // sha:8a576772defc4006637b27e7b0bef2c8bb6f3f7465d27426f1684da58ea9f969
                .size(42_u64)
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
                .mock("GET", "/v2/mockserver/test_package/manifests/sha256:bc669544845542470042912a0f61b90499ffc2320b45ea66b0be50439c5aab19")
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
                .mock("GET", "/v2/mockserver/test_package/blobs/sha256:8a576772defc4006637b27e7b0bef2c8bb6f3f7465d27426f1684da58ea9f969")
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

        let env = Env {
            path: Some("/foo".to_string()),
            ..Env::default()
        };
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/foo/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, blob);
    }

    #[tokio::test]
    async fn download_package_invalid_file() {
        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri("http://localhost.unittest/wp/mockserver/test_package/.env")
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, "Unkown filetype '.env'");
    }

    #[tokio::test]
    async fn download_package_invalid_whl() {
        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri("http://localhost.unittest/wp/mockserver/test_package/foo.whl")
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, "Invalid binary distribution filename 'foo.whl'");
    }

    #[tokio::test]
    async fn download_package_invalid_tar() {
        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri("http://localhost.unittest/wp/mockserver/test_package/foo.tar.gz")
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .into(),
        )
        .unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, "Invalid source distribution filename 'foo.tar.gz'");
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
                    .digest(digest("FooBar"))
                    .size(6_u64)
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
                    .digest(digest("manifest-digest")) // sha256:bc669544845542470042912a0f61b90499ffc2320b45ea66b0be50439c5aab19
                    .size(6_u64)
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
                .mock("GET", "/v2/mockserver/test_package/manifests/sha256:bc669544845542470042912a0f61b90499ffc2320b45ea66b0be50439c5aab19")
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

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
                .digest(digest("FooBar"))
                .size(6_u64)
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

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri(format!(
                "http://localhost.unittest/{encoded_url}/mockserver/test_package/test_package-0.1.0.tar.gz"
            ))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
    async fn delete_package() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest(digest("mani1")) // sha256:81cbc3714a310e6a05cfab0000b1e58ddbf160b6e611b18fa532f19859eafe85
                    .size(6_u64)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".tar.gz".to_string()))
                            .os(Os::Other("any".to_string()))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest(digest("mani2")) // sha256:f7e24eba171386f4939a205235f3ab0dc3b408368dbd3f3f106ddb9e05a32198
                    .size(6_u64)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".whl".to_string()))
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
            // Delete 0.1.0 mani1 manifest
            server
                .mock("DELETE", "/v2/mockserver/test_package/manifests/sha256:81cbc3714a310e6a05cfab0000b1e58ddbf160b6e611b18fa532f19859eafe85")
                .with_status(202)
                .create_async()
                .await,
            // Delete 0.1.0 mani2 manifest
            server
                .mock("DELETE", "/v2/mockserver/test_package/manifests/sha256:f7e24eba171386f4939a205235f3ab0dc3b408368dbd3f3f106ddb9e05a32198")
                .with_status(202)
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("DELETE")
            .uri(format!("/{encoded_url}/mockserver/test-package/0.1.0"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
        assert_eq!(body, "Deleted");
    }

    #[tokio::test]
    async fn delete_package_subpath() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let encoded_url = urlencoding::encode(&url).into_owned();

        let index_010 = ImageIndexBuilder::default()
            .schema_version(2_u32)
            .media_type("application/vnd.oci.image.index.v1+json")
            .artifact_type("application/pyoci.package.v1")
            .manifests(vec![
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest(digest("mani1")) // sha256:81cbc3714a310e6a05cfab0000b1e58ddbf160b6e611b18fa532f19859eafe85
                    .size(6_u64)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".tar.gz".to_string()))
                            .os(Os::Other("any".to_string()))
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
                DescriptorBuilder::default()
                    .media_type("application/vnd.oci.image.manifest.v1+json")
                    .digest(digest("mani2")) // sha256:f7e24eba171386f4939a205235f3ab0dc3b408368dbd3f3f106ddb9e05a32198
                    .size(6_u64)
                    .platform(
                        PlatformBuilder::default()
                            .architecture(Arch::Other(".whl".to_string()))
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
            // Delete 0.1.0 mani1 manifest
            server
                .mock("DELETE", "/v2/mockserver/test_package/manifests/sha256:81cbc3714a310e6a05cfab0000b1e58ddbf160b6e611b18fa532f19859eafe85")
                .with_status(202)
                .create_async()
                .await,
            // Delete 0.1.0 mani2 manifest
            server
                .mock("DELETE", "/v2/mockserver/test_package/manifests/sha256:f7e24eba171386f4939a205235f3ab0dc3b408368dbd3f3f106ddb9e05a32198")
                .with_status(202)
                .create_async()
                .await,
            server
                .mock("GET", mockito::Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        ];

        let env = Env {
            path: Some("/foo".to_string()),
            ..Env::default()
        };
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("DELETE")
            .uri(format!("/foo/{encoded_url}/mockserver/test-package/0.1.0"))
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

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
        assert_eq!(body, "Deleted");
    }

    #[tokio::test]
    async fn health() {
        let env = Env::default();
        let service = pyoci_service(&env);
        let req = Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(req).await.unwrap();

        let status = response.status();
        assert_eq!(status, StatusCode::OK);
    }

    #[test]
    fn router_empty_subpath() {
        let _ = router(&Env {
            path: clean_subpath(Some(String::new())),
            ..Env::default()
        });
    }
}
