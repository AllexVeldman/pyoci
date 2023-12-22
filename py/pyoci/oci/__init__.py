"""OCI client library for Python

This module provides a Python API for a subset of the OCI registry API.
"""
import logging
from copy import copy
from pathlib import Path
from typing import Generator

from .client import Client
from .config import EmptyConfig
from .index import Index
from .layer import Layer
from .manifest import Manifest
from .package import PackageInfo

logger = logging.getLogger(__name__)

ARTIFACT_TYPE = "application/pyoci.package.v1"


def publish_package(path: Path, client: Client, namespace: str = ""):
    """Publish a Python package to an OCI registry

    :param name: The name of the package.
    :param path: The path to the package file.
    :param client: The OCI client to use.

    """
    if not path.is_file():
        raise ValueError("path should point to an existing file")
    package = PackageInfo.from_path(path, namespace=namespace)

    logger.info("Publishing %s", package)

    index = Index.pull(
        name=package.name,
        reference=package.version,
        artifact_type=ARTIFACT_TYPE,
        client=client,
    )
    manifest = Manifest(
        artifactType=ARTIFACT_TYPE,
        config=EmptyConfig(),
    )
    manifest.layers.append(
        Layer.from_path(
            package.file_path,
            artifact_type=ARTIFACT_TYPE,
        )
    )
    index.add_manifest(
        manifest,
        platform=package.platform(),
    )
    index.push(client=client)


def list_package_version(
    package: PackageInfo, client: Client
) -> Generator[PackageInfo, None, None]:
    """List the available versions of a package"""
    index = Index.pull(
        name=package.name,
        reference=package.version,
        artifact_type=ARTIFACT_TYPE,
        client=client,
    )
    for manifest in index.manifests:
        package_alt = copy(package)
        package_alt.architecture = manifest.platform.architecture
        yield package_alt


def list_package(package: PackageInfo, client: Client):
    """List all available files for a package"""
    result = client.list(name=package.name)
    return [
        str(file)
        for tag in result["tags"]
        for file in list_package_version(
            package=PackageInfo(package.distribution, tag, namespace=package.namespace),
            client=client,
        )
    ]


def pull_package(package: PackageInfo, client: Client):
    """Pull a specific package from an OCI registry"""
    index = Index.pull(
        name=package.name,
        reference=package.version,
        artifact_type=ARTIFACT_TYPE,
        client=client,
    )
    for manifest_descr in index.manifests:
        if manifest_descr.platform.architecture == package.architecture:
            manifest = Manifest.from_descriptor(
                name=index.name, descriptor=manifest_descr, client=client
            )
            break
    else:
        raise ValueError("Unknown package %s", package)

    if manifest.artifactType != ARTIFACT_TYPE:
        raise ValueError("Unknown artifact type %s", manifest.artifactType)

    layer = manifest.layers[0]
    return layer.download(name=index.name, client=client)
