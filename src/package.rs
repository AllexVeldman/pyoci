use std::marker::PhantomData;

use anyhow::{bail, Result};
use http::StatusCode;

use crate::pyoci::PyOciError;

pub trait FileState {}

pub struct WithFile;
pub struct WithoutFile;

impl FileState for WithFile {}
impl FileState for WithoutFile {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Package<'a, T: FileState> {
    registry: &'a str,
    namespace: &'a str,
    name: String,
    version: Option<String>,
    arch: Option<String>,
    _phantom: PhantomData<T>,
}

/// Create a Package without version or file information.
pub fn new<'a>(registry: &'a str, namespace: &'a str, name: &'a str) -> Package<'a, WithoutFile> {
    let name = name.replace('-', "_");
    Package {
        registry,
        namespace,
        name,
        version: None,
        arch: None,
        _phantom: PhantomData,
    }
}

/// Create a Package parsing a filename into it's components
///
/// The filename is expected to be normalized, specifically there should be no '-' in any of
/// it's components.
/// ref: https://packaging.python.org/en/latest/specifications/binary-distribution-format/#escaping-and-unicode
pub fn from_filename<'a>(
    registry: &'a str,
    namespace: &'a str,
    filename: &str,
) -> Result<Package<'a, WithFile>> {
    if filename.is_empty() {
        bail!("Empty filename")
    }
    if filename == "json" {
        Err(PyOciError::from((
            StatusCode::NOT_FOUND,
            format!("Unkown filetype '{}'", filename),
        )))?;
    }
    let (name, version, arch) = match filename.strip_suffix(".tar.gz") {
        Some(rest) => match rest.splitn(2, '-').collect::<Vec<_>>()[..] {
            [name, version] => (name, version, ".tar.gz"),
            _ => bail!("Invalid source distribution filename '{}'", filename),
        },
        None => match filename.splitn(3, '-').collect::<Vec<_>>()[..] {
            [name, version, arch] => (name, version, arch),
            _ => bail!("Invalid binary distribution filename '{}'", filename),
        },
    };
    Ok(Package {
        registry,
        namespace,
        name: name.to_string(),
        version: Some(version.to_string()),
        arch: Some(arch.to_string()),
        _phantom: PhantomData,
    })
}

impl<T: FileState> Package<'_, T> {
    /// Add/replace the version and architecture of the package for OCI provided values
    ///
    /// Replaces '-' by '+' to get back to the python definition of the version
    ///
    /// <reference> as a tag MUST be at most 128 characters in length and MUST match the following regular expression:
    /// [a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}
    /// <https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pulling-manifests>
    pub fn with_oci_file(&self, tag: &str, arch: &str) -> Package<WithFile> {
        Package {
            registry: self.registry,
            namespace: self.namespace,
            name: self.name.to_owned(),
            version: Some(tag.replace('-', "+")),
            arch: Some(arch.to_string()),
            _phantom: PhantomData,
        }
    }

    /// Name of the package
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Name of the package as used for the OCI registry
    ///
    /// The package is in the format `<namespace>/<name>`
    /// Returns an error when the package name is not set
    pub fn oci_name(&self) -> String {
        format!("{}/{}", self.namespace, self.name).to_lowercase()
    }

    pub fn registry(&self) -> Result<url::Url> {
        registry_url(self.registry)
    }
}

/// Parse the registry URL
///
/// If no scheme is provided, it will default to `https://`
/// To call an HTTP registry, the scheme must be provided as a url-encoded string.
/// Example: `http://localhost:5000` -> `http%3A%2F%2Flocalhost%3A5000`
fn registry_url(registry: &str) -> Result<url::Url> {
    let registry = urlencoding::decode(registry)?;
    let registry = if registry.starts_with("http://") || registry.starts_with("https://") {
        registry.into_owned()
    } else {
        format!("https://{}", registry)
    };

    let url = url::Url::parse(&registry)?;
    Ok(url)
}

impl Package<'_, WithFile> {
    /// Tag of the package as used for the OCI registry
    pub fn oci_tag(&self) -> String {
        // OCI tags are not allowed to contain a "+" character
        // python versions can't contain a "-" character
        // Replace the "+" from the python version with a "-" in the OCI version
        self.version.as_ref().unwrap().replace('+', "-")
    }

    /// Architecture of the package as used for the OCI registry
    pub fn oci_architecture(&self) -> &str {
        self.arch.as_ref().unwrap()
    }

    /// Relative uri for this package
    pub fn py_uri(&self) -> String {
        // We assume https on all endpoints if the scheme is not provided
        // This prevents url encoding the scheme in the default case
        // It also makes the default work when running behind proxies that
        // decode the URL before hitting the server, like azure.
        // https://learn.microsoft.com/en-us/answers/questions/1160320/azure-is-decoding-characters-in-the-url-before-rea
        let registry = self
            .registry
            .strip_prefix("https://")
            .unwrap_or(self.registry);
        let registry = urlencoding::encode(registry);
        format!(
            "/{}/{}/{}/{}",
            registry,
            self.namespace,
            self.name,
            self.filename()
        )
    }

    /// Return the filename of this package
    /// Returns an empty string if we have no file information
    pub fn filename(&self) -> String {
        let version = self.version.as_ref().unwrap();
        let arch = self.arch.as_ref().unwrap();
        match arch.ends_with(".whl") {
            true => format!("{}-{}-{}", self.name, version, arch),
            false => format!("{}-{}{}", self.name, version, arch),
        }
    }
}

#[cfg(test)]
mod tests {

    use test_case::test_case;

    use super::*;

    #[test]
    fn test_registry_url() {
        assert_eq!(
            registry_url("foo.io").unwrap(),
            url::Url::parse("https://foo.io").unwrap()
        );
        assert_eq!(
            registry_url("http://foo.io").unwrap(),
            url::Url::parse("http://foo.io").unwrap()
        );
        assert_eq!(
            registry_url("https://foo.io").unwrap(),
            url::Url::parse("https://foo.io").unwrap()
        );
        assert_eq!(
            registry_url("http://localhost:5000").unwrap(),
            url::Url::parse("http://localhost:5000").unwrap()
        );
        assert_eq!(
            registry_url("http%3A%2F%2Flocalhost%3A5000").unwrap(),
            url::Url::parse("http://localhost:5000").unwrap()
        );
    }

    #[test]
    /// Test if we can get the package OCI name (namespace/name)
    fn test_info_oci_name() {
        let info = new("https://foo.example", "bar", "baz");
        assert_eq!(info.oci_name(), "bar/baz".to_string());
    }

    /// Test if we can get the package OCI tag (version)
    /// OCI tags are not allowed to contain a "+" character
    #[test_case("bar-1.tar.gz", "1"; "major version")]
    #[test_case("bar-1.0.0.tar.gz", "1.0.0"; "simple version")]
    #[test_case("bar-1.0.0.dev4+g1664eb2.d20231017.tar.gz", "1.0.0.dev4-g1664eb2.d20231017"; "full version")]
    fn test_info_oci_tag(filename: &str, expected: &str) {
        let info = from_filename("https://foo.example", "bar", filename).unwrap();
        assert_eq!(info.oci_tag(), expected.to_string());
    }

    #[test]
    /// Test if Info.py_uri() url-encodes the registry
    fn test_info_py_uri() {
        let info = from_filename("https://foo.example:4000", "bar", "baz-1.tar.gz").unwrap();
        assert_eq!(
            info.py_uri(),
            "/foo.example%3A4000/bar/baz/baz-1.tar.gz".to_string()
        );
    }

    #[test]
    /// Test Info.with_oci_file() return an Info object with the new version
    fn test_info_with_oci_file() {
        let info = new("https://foo.example", "bar", "baz");
        let info = info.with_oci_file("0.1.pre3-1234.foobar", "tar.gz");
        assert_eq!(info.version, Some("0.1.pre3+1234.foobar".to_string()));
    }

    #[test_case("baz-1-cp311-cp311-macosx_13_0_x86_64.whl"; "wheel simple version")]
    #[test_case("baz-2.5.1.dev4+g1664eb2.d20231017-1234-cp311-cp311-macosx_13_0_x86_64.whl"; "wheel full version")]
    #[test_case("baz-1.tar.gz"; "sdist simple version")]
    #[test_case("baz-2.5.1.dev4+g1664eb2.d20231017.tar.gz"; "sdist full version")]
    /// Test if we can convert from and to filenames
    fn test_info_filename(input: &str) {
        let obj = from_filename("foo", "bar", input).unwrap();
        assert_eq!(obj.filename(), input);
    }
}
