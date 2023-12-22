use std::{error, fmt, path::Path, str::FromStr};

use regex::Regex;

#[derive(Debug)]
pub enum ParseError {
    EmptyString,
    NotAFile,
    /// Package name does not comply to the python packaging naming conventions
    InvalidPackageName,
    /// Filename has an unsupported extension
    UnknownFileType(String),
    /// Name of the package in the URL does not match the package name in the filename
    NameMismatch,
    /// Failed to parse URL
    UrlError,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse")
    }
}
impl error::Error for ParseError {}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct File {
    pub name: String,
    pub version: Option<String>,
    extension: Option<String>,
    build_tag: Option<String>,
    python_tag: Option<String>,
    abi_tag: Option<String>,
    platform_tag: Option<String>,
}

impl FromStr for File {
    type Err = ParseError;

    /// Parse
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(ParseError::EmptyString);
        };
        let extension = match Path::new(value).extension() {
            Some(ext) => ext.to_str(),
            None => None,
        };
        match extension {
            Some("whl") => {
                let re = Regex::new(
                    r"(?x)
                    ^(?P<distribution>[a-z0-9][a-z0-9_]*[a-z0-9])
                    -(?P<version>[0-9a-z.+]+)
                    (?:-(?P<build>[\w_]+))??
                    -(?P<python>[\w_]+)
                    -(?P<abi>[\w_]+)
                    -(?P<platform>[\w_]+)
                    (?P<extension>\.whl)
                    ",
                )
                .unwrap();
                match re.captures(value) {
                    Some(capture) => Ok(File {
                        name: capture.name("distribution").unwrap().as_str().to_string(),
                        version: Some(capture.name("version").unwrap().as_str().to_string()),
                        extension: Some(capture.name("extension").unwrap().as_str().to_string()),
                        build_tag: capture
                            .name("build")
                            .map(|build| build.as_str().to_string()),
                        python_tag: Some(capture.name("python").unwrap().as_str().to_string()),
                        abi_tag: Some(capture.name("abi").unwrap().as_str().to_string()),
                        platform_tag: Some(capture.name("platform").unwrap().as_str().to_string()),
                    }),
                    None => Err(ParseError::InvalidPackageName),
                }
            }
            Some("gz") => {
                let re = Regex::new(r"^(?P<distribution>[a-z0-9][a-z0-9_]*[a-z0-9])-(?P<version>[0-9a-z.+]+)(?P<extension>\.tar\.gz)").unwrap();
                match re.captures(value) {
                    Some(capture) => Ok(File {
                        name: capture.name("distribution").unwrap().as_str().to_string(),
                        version: Some(capture.name("version").unwrap().as_str().to_string()),
                        extension: Some(capture.name("extension").unwrap().as_str().to_string()),
                        ..File::default()
                    }),
                    None => Err(ParseError::InvalidPackageName),
                }
            }
            None => Err(ParseError::NotAFile),
            Some(ext) => Err(ParseError::UnknownFileType(ext.to_string())),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Info {
    /// The OCI registry url
    pub registry: String,
    /// The package namespace in the OCI registry
    pub namespace: String,
    /// Python package file attributes
    pub file: File,
}

impl FromStr for Info {
    type Err = ParseError;

    /// Parse a string into Info
    ///
    /// The `filename` and `distribution` parts of the string are expected to already have gone through normalisation
    /// as described by the python packaging standard.
    ///
    /// refs:
    /// - <https://packaging.python.org/en/latest/specifications/name-normalization/#name-normalization>
    /// - <https://packaging.python.org/en/latest/specifications/source-distribution-format/#source-distribution-file-name>
    /// - <https://packaging.python.org/en/latest/specifications/binary-distribution-format/#file-name-convention>
    /// - <https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pulling-manifests>
    ///
    /// Supported formats:
    /// - `<registry>/<namespace>/<filename>`
    /// - `<registry>/<namespace>/<distribution>`
    /// - `<registry>/<namespace>/<distribution>/<filename>`
    fn from_str(value: &str) -> Result<Self, ParseError> {
        let value = value.trim();

        let parts: Vec<&str> = value.split('/').collect();
        match parts[..] {
            [registry, namespace, distribution] => {
                let file = File {
                    name: distribution.to_string(),
                    ..File::default()
                };
                Ok(Info {
                    registry: registry.to_string(),
                    namespace: namespace.to_string(),
                    file,
                })
            }
            [registry, namespace, distribution, filename] => {
                let file = File::from_str(filename)?;
                if distribution != file.name {
                    return Err(ParseError::NameMismatch);
                };
                Ok(Info {
                    registry: registry.to_string(),
                    namespace: namespace.to_string(),
                    file,
                })
            }
            _ => Err(ParseError::UrlError),
        }
    }
}

impl Info {
    /// Name of the package as used for the OCI registry
    pub fn oci_name(&self) -> String {
        format!("{}/{}", self.namespace, self.file.name).to_lowercase()
    }

    pub fn set_version(&mut self, version: &str){
        self.file = File{version: Some(version.to_string()), ..self.file.clone()};
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use test_case::test_case;

    use super::{File, Info};

    #[test_case("foo.io/bar/baz",
        &Info{
            registry: "foo.io".to_string(),
            namespace: "bar".to_string(),
            file: File{
                name:"baz".to_string(),
                ..File::default()}}
        ; "with package")]
    #[test_case("foo.io/bar/baz/baz-1-cp311-cp311-macosx_13_0_x86_64.whl",
        &Info{
            registry: "foo.io".to_string(),
            namespace: "bar".to_string(),
            file: File{
                name: "baz".to_string(),
                version: Some("1".to_string()),
                extension: Some(".whl".to_string()),
                build_tag: None,
                python_tag: Some("cp311".to_string()),
                abi_tag: Some("cp311".to_string()),
                platform_tag: Some("macosx_13_0_x86_64".to_string()),
            }}
    ; "with wheel, minimal")]
    #[test_case("foo.io/bar/baz/baz-2.5.1.dev4+g1664eb2.d20231017-1234-cp311-cp311-macosx_13_0_x86_64.whl",
        &Info{
            registry: "foo.io".to_string(),
            namespace: "bar".to_string(),
            file: File{
                name: "baz".to_string(),
                version: Some("2.5.1.dev4+g1664eb2.d20231017".to_string()),
                extension: Some(".whl".to_string()),
                build_tag: Some("1234".to_string()),
                python_tag: Some("cp311".to_string()),
                abi_tag: Some("cp311".to_string()),
                platform_tag: Some("macosx_13_0_x86_64".to_string()),
            }}
        ; "with wheel, full")]
    #[test_case("foo.io/bar/baz/baz-1.tar.gz",
        &Info{
            registry: "foo.io".to_string(),
            namespace: "bar".to_string(),
            file: File{
                name: "baz".to_string(),
                version: Some("1".to_string()),
                extension: Some(".tar.gz".to_string()),
                ..File::default()}}
    ; "with sdist")]
    /// Test if we can parse a sting into a package::Info object
    fn info_try_from(input: &str, expected: &Info) {
        assert_eq!(Info::from_str(input).unwrap(), *expected);
    }

    #[test]
    /// Test if we can get the package OCI name (namespace/name)
    fn info_oci_name() {
        let into = Info {
            registry: "foo.example".to_string(),
            namespace: "bar".to_string(),
            file: File {
                name: "baz".to_string(),
                ..File::default()
            },
        };
        assert_eq!(into.oci_name(), "bar/baz".to_string());
    }
}
