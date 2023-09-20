import base64
from hashlib import sha256
from pathlib import Path

import requests

from pyoci.manifest import Manifest


class Client:
    """Client for the OCI registry API."""

    def __init__(self, registry_url: str):
        self.registry_url = registry_url
        self._session = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    @property
    def session(self):
        if self._session is None:
            self._session = requests.Session()
        return self._session

    def head(self, uri, **kwargs):
        return self.session.head(f"{self.registry_url}{uri}", **kwargs)

    def get(self, uri, **kwargs):
        return self.session.get(f"{self.registry_url}{uri}", **kwargs)

    def post(self, uri, **kwargs):
        return self.session.post(f"{self.registry_url}{uri}", **kwargs)

    def put(self, uri, **kwargs):
        return self.session.put(f"{self.registry_url}{uri}", **kwargs)

    def close(self):
        if self._session is not None:
            self._session.close()
            self._session = None

    def list(self, name: str) -> dict:
        uri = f"/v2/{name}/tags/list"
        result = self.get(uri)
        result.raise_for_status()
        return result.json()

    def pull_manifest(self, name, reference):
        uri = f"/v2/{name}/manifests/{reference}"
        result = self.get(
            uri, headers={"Accept": "application/vnd.oci.image.manifest.v1+json"}
        )
        if result.status_code == 403:
            print(result.headers)
        result.raise_for_status()
        return result.json()

    def pull_blob(self, name, digest):
        uri = f"/v2/{name}/blobs/{digest}"
        result = self.get(uri)
        result.raise_for_status()
        return result.content

    def push_blob(self, name: str, blob: bytes, digest: str):
        """Push a blob for repository `name`"""
        # response = self.head(f"/v2/{name}/blobs/{digest}")
        # if response.status_code == 200:
        #     print(f"Blob already exists: {name}:{digest}")
        #     return

        uri = f"/v2/{name}/blobs/uploads/?digest={digest}"
        response = self.post(uri, headers={"content-type": "application/octet-stream"})
        response.raise_for_status()
        if response.status_code == 202:
            location = response.headers["location"]
            response = self.session.put(
                url=location,
                data=blob,
                headers={"content-type": "application/octet-stream"},
                params={"digest": digest},
            )
            if response.status_code == 404:
                print(response.json())
            response.raise_for_status()

    def push_manifest(self, name: str, reference: str, manifest: Manifest):
        """Push a manifest for repository `name`"""
        uri = f"/v2/{name}/manifests/{reference}"
        response = self.put(
            uri,
            data=manifest.json(),
            headers={"content-type": manifest.mediaType},
        )
        if "application/json" in response.headers["Content-Type"]:
            print(response.json())
        response.raise_for_status()
