use std::{error, fmt, str::FromStr};


#[derive(Debug)]
pub enum ParseError {
    EmptyString,
    /// Package name does not comply to the python packaging naming conventions
    InvalidPackageName(String),
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

#[derive(Debug, PartialEq, Eq, Clone)]
enum DistType {
    Sdist,
    Wheel,
    Unknown(String),
}

impl Default for DistType {
    fn default() -> Self {
        DistType::Unknown(String::new())
    }
}

impl From<&str> for DistType {

    fn from(s: &str) -> Self {
        match s {
            ".whl" => DistType::Wheel,
            ".tar.gz" => DistType::Sdist,
            unknown => DistType::Unknown(unknown.to_string()),
        }
    }
}

/// Container for a python package filename
/// Supports wheel and sdist filenames
#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct File {
    /// package name
    pub name: String,
    /// package version
    pub version: String,
    /// package architecture
    /// only applicable for dist_type: DistType::Wheel
    architecture: Option<String>,
    /// package distribution type
    dist_type: DistType,
}

impl File {

    /// Replace the version, consumes self
    pub fn with_version(self, version: &str) -> Self {
        File{version: version.to_string(), ..self}
    }

    /// Add/replace the architecture and dist_type
    /// returns a new File instance, consuming self.
    /// accepts the remainder of a python package filename after the version part
    pub fn with_architecture(self, architecture: &str) -> Result<Self, ParseError> {
        match DistType::from(architecture) {
            DistType::Sdist => {
                Ok(File{name: self.name, version: self.version, dist_type: DistType::Sdist, ..File::default()})
            }
            _ => {
                File::from_str(&format!("{}-{}-{}",  &self.name, &self.version, architecture))
            },
        }
    }
    
    /// Return the architecture string as used on the OCI side
    pub fn architecture(&self) -> String {
        match &self.dist_type {
            DistType::Sdist => { ".tar.gz".to_string() },
            DistType::Wheel => { format!("{}.whl",  &self.architecture.as_ref().unwrap())},
            DistType::Unknown(unknown) => { unknown.to_string() }
        }
    }

    /// Return True if this File can be used as an OCI reference
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && !self.version.is_empty()
    }
}

impl FromStr for File {
    type Err = ParseError;

    /// Parse
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(ParseError::EmptyString);
        };
        if let Some(value) = value.strip_suffix(".whl") {
            // Select the str without the extention and split on "-" 3 times
            match value.splitn(3,'-').collect::<Vec<&str>>()[..] {
                [name, version, architecture] => {
                    Ok(File{
                        name: name.to_string(),
                        version: version.to_string(),
                        architecture: Some(architecture.to_string()),
                        dist_type: DistType::Wheel,
                    })
                },
                _ => {Err(ParseError::InvalidPackageName(value.to_string()))}
            }
        }
        else if let Some(value) = value.strip_suffix(".tar.gz") {
            // Select the str without the extention and split on "-" 2 times
            match value.splitn(2,'-').collect::<Vec<&str>>()[..] {
                [name, version] => {
                    Ok(File{
                        name: name.to_string(),
                        version: version.to_string(),
                        architecture: None,
                        dist_type: DistType::Sdist,
                    })
                },
                _ => {Err(ParseError::InvalidPackageName(value.to_string()))}
            }
        }
        else {
            Err(ParseError::UnknownFileType(value.to_string()))
        }
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.dist_type {
            DistType::Sdist => {
                write!(f, "{}-{}.tar.gz", self.name, self.version)
            },
            DistType::Wheel => {
                write!(f, "{}-{}-{}.whl", self.name, self.version, self.architecture.as_ref().unwrap())
            },
            DistType::Unknown(_) => { Err(fmt::Error) }
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
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use test_case::test_case;

    use super::*;

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
                version: "1".to_string(),
                dist_type: DistType::Wheel,
                architecture: Some("cp311-cp311-macosx_13_0_x86_64".to_string()),
            }}
    ; "with wheel, minimal")]
    #[test_case("foo.io/bar/baz/baz-2.5.1.dev4+g1664eb2.d20231017-1234-cp311-cp311-macosx_13_0_x86_64.whl",
        &Info{
            registry: "foo.io".to_string(),
            namespace: "bar".to_string(),
            file: File{
                name: "baz".to_string(),
                version: "2.5.1.dev4+g1664eb2.d20231017".to_string(),
                dist_type: DistType::Wheel,
                architecture: Some("1234-cp311-cp311-macosx_13_0_x86_64".to_string())
            }}
        ; "with wheel, full")]
    #[test_case("foo.io/bar/baz/baz-1.tar.gz",
        &Info{
            registry: "foo.io".to_string(),
            namespace: "bar".to_string(),
            file: File{
                name: "baz".to_string(),
                version: "1".to_string(),
                dist_type: DistType::Sdist,
                ..File::default()}}
    ; "with sdist")]
    /// Test if we can parse a sting into a package::Info object
    fn info_try_from(input: &str, expected: &Info) {
        assert_eq!(Info::from_str(input).unwrap(), *expected);
    }

    #[test_case("baz-1-cp311-cp311-macosx_13_0_x86_64.whl"; "wheel simple version")]
    #[test_case("baz-2.5.1.dev4+g1664eb2.d20231017-1234-cp311-cp311-macosx_13_0_x86_64.whl"; "wheel full version")]
    #[test_case("baz-1.tar.gz"; "sdist simple version")]
    #[test_case("baz-2.5.1.dev4+g1664eb2.d20231017.tar.gz"; "sdist full version")]
    /// Test if a File an be serialized into a string
    fn file_display(input: &str) {
        let obj = File::from_str(input).unwrap();
        assert_eq!(obj.to_string(), input);
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
