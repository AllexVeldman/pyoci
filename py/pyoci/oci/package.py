import re
from dataclasses import InitVar, dataclass, field
from pathlib import Path
from typing import Optional

from pyoci.oci.index import Platform

TAR_GZ = ".tar.gz"

# Normalized distribution
DIST_PATTERN = r"[a-z0-9][a-z0-9_]*[a-z0-9]"
VERSION_PATTERN = r"[0-9a-z.+]+"
SOURCE_PATTERN = (
    rf"^(?P<source_distribution>{DIST_PATTERN})"
    rf"-(?P<source_version>{VERSION_PATTERN})"
    r"(?P<source_ext>\.tar\.gz)$"
)
WHEEL_ARCH_PATTERN = (
    r"(?:-(?P<build>[\w_]+))??"
    r"-(?P<python>[\w_]+)"
    r"-(?P<abi>[\w_]+)"
    r"-(?P<platform>[\w_]+)"
    r"(?P<wheel_ext>.whl)"
)
WHEEL_PATTERN = (
    rf"^(?P<wheel_distribution>{DIST_PATTERN})"
    rf"-(?P<wheel_version>{VERSION_PATTERN})"
    rf"{WHEEL_ARCH_PATTERN}$"
)
FILE_PATTERN = rf"({SOURCE_PATTERN}|{WHEEL_PATTERN})"
FILE_RE = re.compile(FILE_PATTERN)


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
    extension: InitVar[str] = None
    _extension: str | None = field(init=False)
    build_tag: str | None = None
    python_tag: str | None = None
    abi_tag: str | None = None
    platform_tag: str | None = None
    namespace: str = ""
    file_path: Path | None = field(default=None, compare=False)

    def __post_init__(self, extension: str | None):
        self.version = self.full_version
        # Normalize the distribution name
        # https://packaging.python.org/en/latest/specifications/name-normalization/
        self.distribution = re.sub(r"[-_.]+", "-", self.distribution).lower()
        self._extension = extension

    def __str__(self):
        """Return the PackageInfo as a filename"""
        # https://packaging.python.org/en/latest/specifications/binary-distribution-format/
        normalized = self.distribution.replace("-", "_")
        if self.extension == TAR_GZ:
            return f"{normalized}-{self.full_version}{self.extension}"
        return "-".join([normalized, self.full_version, self.architecture])

    @property
    def name(self):
        """OCI package name"""
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

    @property
    def extension(self):
        """Return the file extension"""
        if self._extension is None:
            raise ValueError("PackageInfo.extension is not set")
        return self._extension

    @property
    def architecture(self):
        """Return the architecture string.

        This is everything except the distribution and version.
        For source distributions this is '.tar.gz'.
        For binary distributions this
        '{build_tag}-?{python_tag}-{abi_tag}-{platform_tag}.whl'.
        """
        if self.extension == TAR_GZ:
            return self.extension
        return (
            f"{self.build_tag}-"
            if self.build_tag
            else ""
            + f"{self.python_tag}-{self.abi_tag}-{self.platform_tag}{self.extension}"
        )

    @architecture.setter
    def architecture(self, value: str):
        """Set the architecture string"""
        if value == TAR_GZ:
            self._extension = value
        else:
            match = re.fullmatch(WHEEL_ARCH_PATTERN, "-" + value)
            if not match:
                raise ValueError(f"Invalid architecture: {value}")
            self.build_tag = match["build"]
            self.python_tag = match["python"]
            self.abi_tag = match["abi"]
            self.platform_tag = match["platform"]
            self._extension = match["wheel_ext"]

    @classmethod
    def from_path(cls, value: Path, namespace: str = "") -> "PackageInfo":
        """Parse a package file-path into a PackageInfo object"""
        package = cls.from_string(value.name, namespace=namespace)
        package.file_path = value
        return package

    @classmethod
    def from_string(cls, value: str, namespace: str = "") -> "PackageInfo":
        """Parse a package filename into a PackageInfo object"""
        match = FILE_RE.fullmatch(value)
        distribution = match["source_distribution"] or match["wheel_distribution"]
        version = match["source_version"] or match["wheel_version"]
        ext = match["source_ext"] or match["wheel_ext"]

        return cls(
            distribution=distribution,
            full_version=version,
            extension=ext,
            build_tag=match["build"],
            python_tag=match["python"],
            abi_tag=match["abi"],
            platform_tag=match["platform"],
            namespace=namespace,
        )

    def platform(self) -> Platform:
        """Return the package info as a Platform object"""
        return Platform(
            architecture=self.architecture,
            # TODO: Provide meaningfull value for OS
            os="any",
        )
