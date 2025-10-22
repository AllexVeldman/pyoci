use http::{Method, Request, Uri};
use tower::Service;

#[derive(Debug, Clone)]
pub struct EncodeNamespace<S> {
    inner: S,
    subpath: Option<String>,
}

impl<S> EncodeNamespace<S> {
    pub fn new(inner: S, subpath: Option<&str>) -> Self {
        EncodeNamespace {
            inner,
            subpath: subpath.map(|v| v.to_owned()),
        }
    }
}

impl<S, Body> Service<Request<Body>> for EncodeNamespace<S>
where
    S: Service<Request<Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let req = urlencode_namespace(req, &self.subpath);
        self.inner.call(req)
    }
}

// Middleware to URL-encode "/" in the namespace part of the URI.
//
// This allows an undefined number of sub-namespaces in the uri, just like
// what the OCI registry should support.
//
// By URL-encoding the namespace we allow Axum Router to route like regular
fn urlencode_namespace<B>(mut req: Request<B>, subpath: &Option<String>) -> Request<B> {
    let Some(uri) = _urlencode_namespace(req.method() == Method::POST, req.uri().path(), subpath)
    else {
        return req;
    };
    *req.uri_mut() = uri;

    tracing::debug!("Rewriten: {}", req.uri());
    req
}

// URL-encode "/" in namespace
// GET:
//  /{registry}/{namespace with extra paths}/{package}/
//  /{registry}/{namespace with extra paths}/{package}/json
//  /{registry}/{namespace with extra paths}/{package}/{filename}
// DELETE:
//  /{registry}/{namespace with extra paths}/{package}/{filename}
// POST:
//  /{registry}/{namespace with extra paths}/
fn _urlencode_namespace(is_post_request: bool, uri: &str, subpath: &Option<String>) -> Option<Uri> {
    let subpath_len = if let Some(value) = subpath {
        value.len()
    } else {
        0
    };
    // Find first two "/" so we can separate the common prefix from the rest of the uri
    let registry_end = subpath_len + findn_slash(2, uri[subpath_len..].char_indices()) + 1;

    // return if we did not reach the expected number of "/"
    if registry_end == subpath_len + 1 || registry_end > uri.len() {
        return None;
    }

    // Find the last 2 (GET/DELETE) or 1 (POST) "/", anything before that is the namespace
    let expected_sep_count = if is_post_request { 1 } else { 2 };
    let namespace_end = findn_slash(expected_sep_count, uri.char_indices().rev());

    // return if we did not reach the expected number of "/"
    if namespace_end == subpath_len || namespace_end < registry_end {
        return None;
    }
    let prefix = &uri[..registry_end];
    let namespace = &uri[registry_end..namespace_end].replace('/', "%2F");
    let postfix = &uri[namespace_end..];
    tracing::debug!("Prefix: {}", prefix);
    tracing::debug!("Namespace: {}", namespace);
    tracing::debug!("Postfix: {}", postfix);

    let Ok(uri) = [prefix, namespace, postfix].concat().parse() else {
        // Since we don't alter the original URI in unpredictable ways,
        // this return should be unreachable.
        return None;
    };
    Some(uri)
}

// Return the byte location in `it` of the nth '/'
fn findn_slash(n: usize, it: impl Iterator<Item = (usize, char)>) -> usize {
    let mut count = 0;
    let mut loc = 0;
    for (i, char) in it {
        if char != '/' {
            continue;
        }
        if count < n {
            count += 1;
        }
        if count == n {
            loc = i;
            break;
        }
    }
    loc
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use http::Request;
    use test_case::test_case;

    #[test_case("GET", None, "/reg/nmsps/package/", "/reg/nmsps/package/"; "list package, no change")]
    #[test_case("GET", None,"/reg/nmsps/package/json", "/reg/nmsps/package/json"; "list package json, no change")]
    #[test_case("GET",None, "/reg/nmsps/package/foo.whl", "/reg/nmsps/package/foo.whl"; "download package, no change")]
    #[test_case("DELETE",None, "/reg/nmsps/package/foo.whl", "/reg/nmsps/package/foo.whl"; "delete package, no change")]
    #[test_case("POST",None, "/reg/nmsps/", "/reg/nmsps/"; "post package, no change")]
    #[test_case("GET",None, "/reg/nmsps/sub-nmsps/package/", "/reg/nmsps%2Fsub-nmsps/package/"; "list package, sub-namespace")]
    #[test_case("GET",None, "/reg/nmsps/sub-nmsps/package/json", "/reg/nmsps%2Fsub-nmsps/package/json"; "list package json, sub-namespace")]
    #[test_case("GET",None, "/reg/nmsps/sub-nmsps/package/foo.whl", "/reg/nmsps%2Fsub-nmsps/package/foo.whl"; "download package, sub-namespace")]
    #[test_case("DELETE",None, "/reg/nmsps/sub-nmsps/package/foo.whl", "/reg/nmsps%2Fsub-nmsps/package/foo.whl"; "delete package, sub-namespace")]
    #[test_case("POST",None, "/reg/nmsps/sub-nmsps/", "/reg/nmsps%2Fsub-nmsps/"; "post package, sub-namespace")]
    #[test_case("GET",None, "/foobarbaz", "/foobarbaz"; "no second slash")]
    #[test_case("GET",None, "/foobarbaz/", "/foobarbaz/"; "no third slash in GET")]
    #[test_case("POST",None, "/foobarbaz/", "/foobarbaz/"; "no third slash in POST")]
    #[test_case("GET",None, "/foobar/baz/", "/foobar/baz/"; "no fourth slash")]
    #[test_case("GET",None, "////////////", "//%2F%2F%2F%2F%2F%2F%2F%2F//"; "only slashes")]
    #[test_case("POST",None, "/foo/bar", "/foo/bar"; "no closing slash")]
    #[test_case("GET",Some("/foo"), "/foo/reg/nmsps/sub-nmsps/package/", "/foo/reg/nmsps%2Fsub-nmsps/package/"; "list package, sub-namespace with subpath")]
    #[test_case("GET",Some("/foo"), "/foo/reg/nmsps/sub-nmsps/package/json", "/foo/reg/nmsps%2Fsub-nmsps/package/json"; "list package json, sub-namespace with subpath")]
    #[test_case("GET",Some("/foo"), "/foo/reg/nmsps/sub-nmsps/package/foo.whl", "/foo/reg/nmsps%2Fsub-nmsps/package/foo.whl"; "download package, sub-namespace with subpath")]
    #[test_case("DELETE",Some("/foo"), "/foo/reg/nmsps/sub-nmsps/package/foo.whl", "/foo/reg/nmsps%2Fsub-nmsps/package/foo.whl"; "delete package, sub-namespace with subpath")]
    #[test_case("POST",Some("/foo"), "/foo/reg/nmsps/sub-nmsps/", "/foo/reg/nmsps%2Fsub-nmsps/"; "post package, sub-namespace with subpath")]
    fn urlencode_namespace(method: &str, prefix: Option<&str>, uri: &str, expected: &str) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            super::urlencode_namespace(req, &prefix.map(|v| v.to_string()))
                .uri()
                .path(),
            expected
        );
    }
}
