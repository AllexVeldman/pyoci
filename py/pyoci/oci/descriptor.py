from pydantic import BaseModel, Field

from pyoci.oci import Client


class Descriptor(BaseModel):
    """
    ref: https://github.com/opencontainers/image-spec/blob/main/descriptor.md
    """

    digest: str
    size: int
    mediaType: str
    urls: list[str] | None = None
    annotations: dict[str, str] | None = None
    artifactType: str | None = None
    data: bytes | None = Field(exclude=True, default=None)

    def push(self, name: str, client: Client):
        if self.data is None:
            raise ValueError(f"Missing {self.__class__.__name__}.data")
        client.push_blob(name=name, blob=self.data, digest=self.digest)

    def pull(self, name: str, client: Client) -> bytes:
        self.data = client.pull_blob(name=name, digest=self.digest)
        return self.data
