import base64
import gzip
from hashlib import sha256
from pathlib import Path

from pyoci.client import Client
from pyoci.descriptor import Descriptor


def create_file_layer(
    f: Path, /, name: str, artifact_type: str, client: Client
) -> Descriptor:
    """Create a new layer containing a single file"""
    if not f.is_file():
        raise ValueError(f"{f} is not a file")
    # Set mtime to 0 to ensure the digest does not change if the file does not change
    zipped = gzip.compress(f.read_bytes(), mtime=0)
    digest = f"sha256:{sha256(zipped).hexdigest()}"

    client.push_blob(name=name, blob=zipped, digest=digest)

    layer = Descriptor(
        mediaType=f"{artifact_type}+gzip",
        digest=digest,
        size=len(zipped),
    )
    return layer
