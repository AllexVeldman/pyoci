import json
from dataclasses import asdict, dataclass, field
from hashlib import sha256
from pathlib import Path

from pyoci.descriptor import Descriptor


def dict_factory(data):
    return dict(x for x in data if x[1] is not None)


@dataclass(slots=True)
class Manifest:
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/manifest.md
    """

    config: Descriptor
    artifactType: str | None = None
    layers: list[Descriptor] = field(default_factory=list)
    subject: Descriptor | None = None
    annotations: dict[str, str] | None = None

    mediaType: str = "application/vnd.oci.image.manifest.v1+json"
    schemaVersion: int = 2

    def as_layer(self, blobs_path: Path):
        """Write self to blob location and return the layer descriptor for use in an index"""
        data = self.json().encode("utf-8")
        digest = sha256(data).hexdigest()
        (blobs_path / "sha256").mkdir(parents=True, exist_ok=True)
        with (blobs_path / "sha256" / digest).open("wb") as f:
            f.write(data)
        layer = Descriptor(
            mediaType=self.mediaType,
            digest=f"sha256:{digest}",
            size=len(data),
        )
        return layer

    def json(self):
        return json.dumps(
            asdict(self, dict_factory=dict_factory), indent=2, sort_keys=True
        )
