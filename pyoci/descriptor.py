from dataclasses import dataclass
from typing import Final


@dataclass(slots=True)
class Descriptor:
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/descriptor.md
    """

    digest: str
    size: int
    mediaType: str
    urls: list[str] | None = None
    annotations: dict[str, str] | None = None
    data: str | None = None
    artifactType: str | None = None


EmptyConfig: Final[Descriptor] = Descriptor(
    mediaType="application/vnd.oci.empty.v1+json",
    digest="sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    size=2,
)
