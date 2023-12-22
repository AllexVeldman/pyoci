from __future__ import annotations

import logging
from typing import TYPE_CHECKING
from urllib.parse import urlparse, urlunparse

import httpx

if TYPE_CHECKING:
    from pyoci.oci.index import Index
    from pyoci.oci.manifest import Manifest

logger = logging.getLogger(__name__)

DOCKER_HUB = "registry-1.docker.io"


class AuthenticationError(Exception):
    """Raised when authentication fails."""


def _clean_url(registry_url: str) -> str:
    parts = urlparse(registry_url)
    if not parts.scheme:
        parts = parts._replace(scheme="https")
    if parts.netloc == "docker.io":
        parts = parts._replace(netloc=DOCKER_HUB)
    return urlunparse(parts)


def _parse_www_auth(www_authenticate: str) -> dict[str, str]:
    """Parse the WWW-Authenticate header"""
    result = {}
    for item in www_authenticate.removeprefix("Bearer ").split(","):
        key, value = item.split("=", 1)
        result[key] = value.strip('"')
    return result


class BearerAuth:
    """Attaches HTTP Bearer Authentication to the given Request object."""

    def __init__(self, token: str):
        self.token = token

    def __call__(self, request):
        request.headers["Authorization"] = f"Bearer {self.token}"
        return request


class Client:
    """Client for the OCI registry API."""

    def __init__(
        self,
        registry_url: str,
        username: str | None = None,
        password: str | None = None,
    ):
        self.registry_url = _clean_url(registry_url)
        self.username = username
        self.password = password
        self._session = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    # TODO: Add error handling
    #   https://github.com/opencontainers/distribution-spec/blob/main/spec.md#error-codes

    @property
    def session(self):
        if self._session is None:
            self._session = httpx.Client(
                follow_redirects=True,
                max_redirects=2,
            )
            self.try_authentication()
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

    def try_authentication(self):
        result = self.get("/v2/")
        if result.status_code == 401:
            www_authenticate = _parse_www_auth(result.headers["WWW-Authenticate"])
            logger.debug(www_authenticate)
            self.authenticate(
                token_url=www_authenticate["realm"],
                service=www_authenticate["service"],
                scope=www_authenticate.get("scope"),
            )
        else:
            result.raise_for_status()

    def authenticate(self, token_url, service, scope):
        """Use the token api with basic authentication to get a token

        ref: https://distribution.github.io/distribution/spec/auth/token/
        """
        if not self.password:
            raise AuthenticationError(
                f"{self.registry_url} requires authentication, "
                f"provide a username and/or password."
            )
        response = self.session.get(
            token_url,
            params={
                "grant_type": "password",
                "service": service,
                "client_id": self.username,
            },
            auth=(self.username, self.password),
        )
        response.raise_for_status()
        self.session.auth = BearerAuth(response.json()["token"])

    def list(self, name: str) -> dict:
        uri = f"/v2/{name}/tags/list"
        result = self.get(uri)
        result.raise_for_status()
        return result.json()

    def pull_manifest(
        self,
        name,
        reference,
        media_type: str = (
            "application/vnd.oci.image.manifest.v1+json, "
            "application/vnd.oci.image.index.v1+json"
        ),
    ):
        uri = f"/v2/{name}/manifests/{reference}"
        result = self.get(
            uri,
            headers={"Accept": media_type},
        )
        if result.status_code == 403:
            logger.debug(result.headers)
        result.raise_for_status()
        return result.json()

    def pull_blob(self, name, digest):
        uri = f"/v2/{name}/blobs/{digest}"
        result = self.get(uri)
        result.raise_for_status()
        return result.content

    def push_blob(self, name: str, blob: bytes, digest: str):
        """Push a blob for repository `name`

        ref: https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pushing-blobs
        """
        # Check if the blob already exists
        response = self.head(f"/v2/{name}/blobs/{digest}")
        if response.status_code == 200:
            logger.info(f"Blob already exists: {name}:{digest}")
            return

        # Push the blob using the POST then PUT method
        response = self.post(
            f"/v2/{name}/blobs/uploads/",
            headers={"content-type": "application/octet-stream"},
        )
        response.raise_for_status()
        if response.status_code == 202:
            location = response.headers["location"]
            if location.startswith("/"):
                # Relative location, add the registry url
                put = self.put
            else:
                # Absolute location, use the location as is
                put = self.session.put
            response = put(
                location,
                data=blob,
                headers={"content-type": "application/octet-stream"},
                params={"digest": digest},
            )
            if response.status_code == 404:
                logger.info(response.json())
            response.raise_for_status()

    def push_manifest(
        self, name: str, manifest: Manifest | Index, reference: str | None = None
    ):
        """Push a manifest for repository `name` and tag `reference`

        ref: https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pushing-manifests
        """
        descriptor = manifest.descriptor
        data = descriptor.data
        uri = f"/v2/{name}/manifests/{reference}"
        if reference is None:
            reference = descriptor.digest
            uri = f"/v2/{name}/manifests/{reference}"
            response = self.head(uri)
            if response.status_code == 200:
                logger.info(f"Manifest already exists: {name}:{reference}")
                return

        logger.debug("Pushing manifest: %s", data)
        response = self.put(
            uri,
            data=data,
            headers={"content-type": descriptor.mediaType},
        )

        if (
            not response.is_success
            and int(response.headers["Content-Length"]) > 0
            and "application/json" in response.headers["Content-Type"]
        ):
            logger.error(response.json())
        response.raise_for_status()
