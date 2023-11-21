"""OCI client library for Python

This module provides a Python API for a subset of the OCI registry API.
"""
import logging
from contextlib import suppress
from dataclasses import asdict
from pathlib import Path

from httpx import HTTPError

from pyoci.oci.client import Client
from pyoci.oci.config import EmptyConfig
from pyoci.oci.index import Index
from pyoci.oci.layer import Layer, create_file_layer
from pyoci.oci.manifest import Manifest
from pyoci.oci.package import PackageInfo

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


def list_package_version(package: PackageInfo, client: Client):
    """List the available versions of a package"""
    index = Index.pull(
        name=package.name,
        reference=package.version,
        artifact_type=ARTIFACT_TYPE,
        client=client,
    )
    return [
        PackageInfo(
            **(asdict(package) | {"architecture": manifest.platform.architecture})
        )
        for manifest in index.manifests
    ]


def list_package(name: str, client: Client, namespace: str = ""):
    """List all available files for a package"""
    result = client.list(name=PackageInfo(name, namespace=namespace).name)
    return [
        str(file)
        for tag in result["tags"]
        for file in list_package_version(
            package=PackageInfo(name, tag, namespace=namespace), client=client
        )
    ]


def pull_package(package: str, client: Client, namespace: str = ""):
    """Pull a specific package from an OCI registry"""
    package = PackageInfo.from_string(package, namespace=namespace)
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
