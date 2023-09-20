import json
from pathlib import Path
from typing import NamedTuple
from urllib.parse import urlparse, urlunparse

from pyoci.client import Client
from pyoci.descriptor import EmptyConfig
from pyoci.layer import create_file_layer
from pyoci.manifest import Manifest

ARTIFACT_TYPE = "application/pyoci.package.v1"
DOCKER_HUB = "registry-1.docker.io"


def run():
    # image_path = Path("out")
    # rmtree(image_path, ignore_errors=True)
    #
    # image_path.mkdir(parents=True, exist_ok=True)
    package = Path("dist") / "pyoci-0.1.0.tar.gz"
    # package = Path("dist") / "pyoci-0.1.0-py3-none-any.whl"

    oci_publish_package(repository="http://localhost:5000", package=package)


class PackageInfo(NamedTuple):
    """Python package name information

    ref source distribution:
        https://packaging.python.org/en/latest/specifications/source-distribution-format/#source-distribution-file-format
    ref binary distribution:
        https://packaging.python.org/en/latest/specifications/binary-distribution-format/#file-format
    """

    distribution: str
    version: str
    build_tag: str | None = None
    python_tag: str | None = None
    abi_tag: str | None = None
    platform_tag: str | None = None

    @classmethod
    def from_path(cls, value: Path) -> "PackageInfo":
        if value.name.endswith(".tar.gz"):
            parts = value.name[: -len(".tar.gz")].split("-")
            return cls(*parts)
        elif value.suffix == ".whl":
            parts = value.name[: -len(".whl")].split("-")
            if len(parts) == 6:
                return cls(*parts)
            if len(parts) == 5:
                return cls(*parts[:2], None, *parts[2:])
        else:
            raise ValueError(f"Unknown package type: {value.name}")


def clean_repository(repository: str) -> str:
    parts = urlparse(repository)
    if not parts.scheme:
        parts = parts._replace(scheme="https")
    if parts.netloc == "docker.io":
        parts = parts._replace(netloc=DOCKER_HUB)
    return urlunparse(parts)


def oci_publish_package(repository: str, package: Path):
    repository = clean_repository(repository)

    manifest = Manifest(
        artifactType=ARTIFACT_TYPE,
        config=EmptyConfig,
    )
    info = PackageInfo.from_path(package)
    with Client(repository) as client:
        client.push_blob(name=info.distribution, blob=b"{}", digest=EmptyConfig.digest)
        manifest.layers.append(
            create_file_layer(
                package,
                name="test",
                artifact_type=ARTIFACT_TYPE,
                client=client,
            )
        )
        client.push_manifest(
            name=info.distribution, reference=info.version, manifest=manifest
        )


def oci_layout(image_path):
    (image_path / "oci-layout").write_text(json.dumps({"imageLayoutVersion": "1.0.0"}))


if __name__ == "__main__":
    run()
