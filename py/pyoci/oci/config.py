import json
from dataclasses import asdict, dataclass
from hashlib import sha256
from typing import Any, Literal

from .client import Client
from .descriptor import Descriptor


class EmptyConfig(Descriptor):
    mediaType: str = "application/vnd.oci.empty.v1+json"
    digest: str = (
        "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
    )
    size: int = 2

    def model_post_init(self, __context: Any) -> None:
        self.data = b"{}"
