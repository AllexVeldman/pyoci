from dataclasses import dataclass, field
from pathlib import Path

from pyoci.oci.index import Platform

TAR_GZ = ".tar.gz"


@dataclass(slots=True)
class PackageInfo:
    """Python package name information

    ref source distribution:
        https://packaging.python.org/en/latest/specifications/source-distribution-format/#source-distribution-file-format
    ref binary distribution:
        https://packaging.python.org/en/latest/specifications/binary-distribution-format/#file-format
    """

    distribution: str
    full_version: str = ""
    architecture: str | None = None
    namespace: str = ""
    file_path: Path | None = field(default=None, compare=False)

    def __post_init__(self):
        self.version = self.full_version

    def __str__(self):
        if self.architecture == TAR_GZ:
            return f"{self.distribution}-{self.full_version}{self.architecture}"
        return "-".join([self.distribution, self.full_version, self.architecture])

    @property
    def name(self):
        return f"{self.namespace}/{self.distribution}".lower()

    @property
    def version(self):
        """Return the clean version string

        Semver strings can contain '<public version>+<local version>' information.
        The "+" is not allowed in the OCI tag names.
        Return the version with "+" replaced by "-".

        Since the package name, version, and architecture are stored separately
        we can reconstruct the original version later.
        """
        return self.full_version.replace("+", "-")

    @version.setter
    def version(self, value):
        self.full_version = value.replace("-", "+")

    @classmethod
    def from_path(cls, value: Path, namespace: str = "") -> "PackageInfo":
        package = cls.from_string(value.name, namespace=namespace)
        package.file_path = value
        return package

    @classmethod
    def from_string(cls, value: str, namespace: str = "") -> "PackageInfo":
        distribution, version, *rest = value.split("-", maxsplit=2)
        if not rest:
            if not version.endswith(TAR_GZ):
                raise ValueError(f"Unknown package type: {value}")
            version, *rest = version[: -len(TAR_GZ)], TAR_GZ
        return cls(distribution, version, *rest, namespace=namespace)

    def platform(self) -> Platform:
        """Return the package info as a Platform object"""
        return Platform(
            architecture=self.architecture,
            # TODO: Provide meaningfull value for OS
            os="any",
        )
