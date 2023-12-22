import gzip
from hashlib import sha256
from pathlib import Path
from typing import BinaryIO

from pyoci.oci.client import Client
from pyoci.oci.descriptor import Descriptor


class Layer(Descriptor):
    @classmethod
    def from_path(cls, path: Path, artifact_type: str):
        """Create a new layer containing a single file"""
        if not path.is_file():
            raise ValueError(f"{path} is not a file")
        # Set mtime to 0 to ensure the digest does not change if the file does not change
        zipped = gzip.compress(path.read_bytes(), mtime=0)
        digest = f"sha256:{sha256(zipped).hexdigest()}"

        return cls(
            mediaType=f"{artifact_type}+gzip",
            digest=digest,
            size=len(zipped),
            data=zipped,
        )

    @classmethod
    def from_file(cls, file: BinaryIO, artifact_type: str):
        """Create a new layer containing a single file"""
        zipped = gzip.compress(file.read(), mtime=0)
        digest = f"sha256:{sha256(zipped).hexdigest()}"

        return cls(
            mediaType=f"{artifact_type}+gzip",
            digest=digest,
            size=len(zipped),
            data=zipped,
        )

    def download(self, name: str, client: Client) -> bytes:
        return gzip.decompress(self.pull(name=name, client=client))
