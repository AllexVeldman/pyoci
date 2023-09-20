import json
from dataclasses import asdict, dataclass, field
from pathlib import Path

from pyoci.descriptor import Descriptor


@dataclass
class Platform:
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/image-index.md
    """

    architecture: str
    os: str
    osVersion: str | None = None
    osFeatures: list[str] | None = None
    variant: str | None = None


@dataclass
class PlatformDescriptor(Descriptor):
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/image-index.md
    """

    mediaType: str = "application/vnd.oci.image.manifest.v1+json"
    platform: Platform | None = None


@dataclass
class Index:
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/image-index.md
    """

    artifactType: str | None = None
    manifests: list[Descriptor] = field(default_factory=list)
    schemaVersion: int = 2
    mediaType: str = "application/vnd.oci.image.index.v1+json"

    def dump(self, path: Path):
        """Dump the index as index.json at path"""
        with (path / "index.json").open("w") as f:
            f.write(self.json())

    def json(self) -> str:
        return json.dumps(
            asdict(
                self,
                dict_factory=lambda data: dict(x for x in data if x[1] is not None),
            ),
            indent=2,
            sort_keys=True,
        )
