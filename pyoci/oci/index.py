import json
import logging
from contextlib import suppress
from dataclasses import asdict, dataclass, field
from functools import cached_property
from hashlib import sha256

from httpx import HTTPError
from pydantic import BaseModel, Field

from pyoci.oci.client import Client
from pyoci.oci.descriptor import Descriptor
from pyoci.oci.manifest import Manifest

logger = logging.getLogger(__name__)


class Platform(BaseModel):
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/image-index.md
    """

    architecture: str
    os: str
    osVersion: str | None = None
    osFeatures: list[str] | None = None
    variant: str | None = None


class PlatformDescriptor(Descriptor):
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/image-index.md
    """

    mediaType: str = "application/vnd.oci.image.manifest.v1+json"
    platform: Platform | None = None
    manifest: Manifest | None = Field(exclude=True, default=None)

    @classmethod
    def from_manifest(
        cls, platform: Platform, manifest: Manifest
    ) -> "PlatformDescriptor":
        descriptor = manifest.descriptor
        return cls(
            digest=descriptor.digest,
            size=descriptor.size,
            platform=platform,
            manifest=manifest,
        )


def dict_factory(data):
    return dict(x for x in data if x[1] is not None)


class Index(BaseModel):
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/image-index.md
    """

    name: str
    reference: str
    artifactType: str | None = None
    manifests: list[PlatformDescriptor] = []
    schemaVersion: int = 2
    mediaType: str = "application/vnd.oci.image.index.v1+json"

    def add_manifest(
        self,
        manifest: Manifest,
        platform: Platform | None = None,
    ):
        platform_descriptor = PlatformDescriptor.from_manifest(
            manifest=manifest, platform=platform
        )
        arch = platform_descriptor.platform.architecture
        manifests = {
            p.platform.architecture: (idx, p) for idx, p in enumerate(self.manifests)
        }
        if arch not in manifests:
            self.manifests.append(platform_descriptor)
        elif manifests[arch][1].digest != platform_descriptor.digest:
            logger.warning(
                "'%s-%s-%s' already exists with different content, overwriting.",
                self.name,
                self.reference,
                arch,
            )
            self.manifests[manifests[arch][0]] = platform_descriptor
        else:
            logger.info(
                "'%s-%s-%s' already exists, skipping.", self.name, self.reference, arch
            )
            self.manifests[manifests[arch][0]] = platform_descriptor

    @classmethod
    def pull(
        cls, name: str, reference: str, artifact_type: str | None, client: Client
    ) -> "Index":
        data = {
            "name": name,
            "reference": reference,
            "artifactType": artifact_type,
        }
        with suppress(HTTPError):
            manifest = client.pull_manifest(name=name, reference=reference)
            logger.debug(manifest)
            if manifest["mediaType"] == "application/vnd.oci.image.index.v1+json":
                data |= manifest
        return Index.model_validate(data)

    def push(self, client: Client):
        for platform in self.manifests:
            if platform.manifest is not None:
                platform.manifest.push(name=self.name, client=client)
        client.push_manifest(name=self.name, reference=self.reference, manifest=self)

    @cached_property
    def descriptor(self) -> Descriptor:
        data = self.model_dump_json(exclude_none=True).encode("utf-8")
        digest = f"sha256:{sha256(data).hexdigest()}"
        return Descriptor(
            mediaType=self.mediaType,
            digest=digest,
            size=len(data),
            data=data,
        )
