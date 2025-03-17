# Design

## ArtifactType
Packages published through PyOCI use the `application/pyoci.package.v1` [artifactType](https://github.com/opencontainers/image-spec/blob/v1.1.0/manifest.md#guidelines-for-artifact-usage).

## Image index
The image index gets tagged with the package version.
This allows multiple build artifacts to be published to the same package version.

`com.pyoci.sha256_digest` is added as an annotation in the `ImageIndex.manifests[]`
containing the digest of the package blob.
This is so we don't need to pull the manifest itself to get the digest of the package.

`com.pyoci.project_urls` is added from the published Project-URLs and included in the `<>/json` endpoint.

```json
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.oci.image.index.v1+json",
  "artifactType": "application/pyoci.package.v1",
  "manifests": [
    {
      "mediaType": "application/vnd.oci.image.manifest.v1+json",
      "digest": "sha256:e281659053054737342fd0c74a7605c4678c227db1e073260b44f845dfdf535a",
      "size": 496,
      "platform": {
        "architecture": ".tar.gz",
        "os": "any"
      },
      "annotations": {
        "org.opencontainers.image.created":"2024-11-20T20:23:36Z",
        "com.pyoci.sha256_digest": "b7513fb69106a855b69153582dec476677b3c79f4a13cfee6fb7a356cfa754c0",
        "com.pyoci.project_urls": "{\"Homepage\":\"https://pyoci.com\",\"Repository\":\"https://github.com/allexveldman/pyoci\"}"
      }
    }
  ],
  "annotations": {
    "org.opencontainers.image.created":"2024-11-20T20:23:36Z"
  }
}
```

## Image Manifest

When a package is published with `PyOci :: Label :: <key> :: <value>` classifiers,
the key/value pair will be added to the ImageManifest annotations.

`org.opencontainers.image.created` will always be added.

```json
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.oci.image.manifest.v1+json",
  "artifactType": "application/pyoci.package.v1",
  "config": {
    "mediaType": "application/vnd.oci.empty.v1+json",
    "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    "size": 2
  },
  "layers": [
    {
      "mediaType": "application/pyoci.package.v1",
      "digest": "sha256:b7513fb69106a855b69153582dec476677b3c79f4a13cfee6fb7a356cfa754c0",
      "size": 22
    }
  ],
  "annotations": {
    "org.opencontainers.image.description": "Published using PyOCI as part of the examples.",
    "org.opencontainers.image.url": "https://github.com/allexveldman/pyoci",
    "org.opencontainers.image.created":"2024-11-20T20:23:36Z"
  }
}
```

# References

### PyPi
- Simple API PEP: https://peps.python.org/pep-0503/
- Simple JSON extention PEP: https://peps.python.org/pep-0691/#content-types
- API definitions: https://docs.pypi.org/api/

### Python packaging
- Name normalization: https://packaging.python.org/en/latest/specifications/name-normalization/#name-normalization
- `.tar.gz`: https://packaging.python.org/en/latest/specifications/source-distribution-format/#source-distribution-file-name
- `.whl`: https://packaging.python.org/en/latest/specifications/binary-distribution-format/#file-name-convention
- Core metadata: https://packaging.python.org/en/latest/specifications/core-metadata/

### OCI
- Token auth: https://distribution.github.io/distribution/spec/auth/token/
- Distribution spec: https://github.com/opencontainers/distribution-spec/blob/main/spec.md
- Image spec: https://github.com/opencontainers/image-spec/blob/main/spec.md

### Other
- WWW-Authenticate header: https://datatracker.ietf.org/doc/html/rfc6750#section-3
