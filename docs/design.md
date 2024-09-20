# Design

## ArtifactType
Packages published through PyOCI use the `application/pyoci.package.v1` [artifactType](https://github.com/opencontainers/image-spec/blob/v1.1.0/manifest.md#guidelines-for-artifact-usage).

## Image index
The image index gets tagged with the package version.
This allows multiple build artifacts to be published to the same package version.

```json
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.oci.image.index.v1+json",
  "artifactType": "application/pyoci.package.v1",
  "manifests": [
    {
      "mediaType": "application/vnd.oci.image.manifest.v1+json",
      "digest": "sha256:7ffd96d9eab411893eeacfa906e30956290a07b0141d7c1dd54c9fd5c7c48cf5",
      "size": 422,
      "platform": {
        "architecture": ".tar.gz",
        "os": "any"
      }
    }
  ]
}
```

## Image Manifest

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
  ]
}
```
