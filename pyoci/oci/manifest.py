import json
from dataclasses import asdict, dataclass, field
from functools import cached_property
from hashlib import sha256

from pydantic import BaseModel

from pyoci.oci.client import Client
from pyoci.oci.descriptor import Descriptor
from pyoci.oci.layer import Layer


class Manifest(BaseModel):
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/manifest.md
    """

    config: Descriptor
    artifactType: str | None = None
    layers: list[Layer] = []
    subject: Descriptor | None = None
    annotations: dict[str, str] | None = None

    mediaType: str = "application/vnd.oci.image.manifest.v1+json"
    schemaVersion: int = 2

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

    @classmethod
    def from_descriptor(
        cls, name: str, descriptor: Descriptor, client: Client
    ) -> "Manifest":
        if descriptor.data is not None:
            return cls.model_validate_json(descriptor.data)
        return cls.model_validate(
            client.pull_manifest(
                name=name, reference=descriptor.digest, media_type=descriptor.mediaType
            )
        )

    def push(self, name: str, client: Client):
        self.config.push(name=name, client=client)
        for layer in self.layers:
            layer.push(name=name, client=client)
        client.push_manifest(name=name, manifest=self)
