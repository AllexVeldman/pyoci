# PyOCI
Publish and download (private) python packages using an OCI registry for storage.

[![Test](https://github.com/AllexVeldman/pyoci/actions/workflows/test.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/test.yaml)
[![Examples](https://github.com/AllexVeldman/pyoci/actions/workflows/examples.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/examples.yaml)
[![Deploy](https://github.com/AllexVeldman/pyoci/actions/workflows/deploy.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/deploy.yaml)
[![codecov](https://codecov.io/github/AllexVeldman/pyoci/graph/badge.svg?token=SXFCPX8M22)](https://codecov.io/github/AllexVeldman/pyoci)

## Why PyOCI
To not have to rely on `yet-another-cloud-provider` for private Python packages, PyOCI, makes `ghcr.io` act like a python index.  
In addition, this completely removes the need for separate access management as GitHub Packages access control applies.

Most subscriptions with cloud providers include an [OCI](https://opencontainers.org/) (docker image) registry where private containers can be published and distributed from.

PyOCI allows using any (private) OCI registry as a python package index, as long as it implements the [OCI distribution specification](https://github.com/opencontainers/distribution-spec/blob/main/spec.md).
It acts as a proxy between pip and the OCI registry.

An instance of PyOCI is available at https://pyoci.com, to use this proxy, please see the [Getting started](#getting-started).

Tested registries:
- [ghcr.io](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
- [Azure Container Registry](https://azure.microsoft.com/en-us/products/container-registry)


Published packages will show up in the OCI registry UI:

<img width="500" alt="ghcr.io hello-world package versions" src="https://github.com/user-attachments/assets/c3595da9-91e7-4ee6-b890-2ed9baca3c9d">
<img width="500" alt="ghcr.io Distinct distributions will show up as separate architectures for the same versions" src="https://github.com/user-attachments/assets/63d130cf-5551-4131-b48b-a6e8f259cbc5">

## Getting started
To install a package with pip using PyOCI:
```commandline
pip install --index-url="http://<username>:<password>@<pyoci-url>/<OCI-registry-url>/<namespace>/" <package-name>
```
- `<pyoci-url>`: https://pyoci.com
- `<OCI-registry-url>`: URL of the OCI registry to use.
- `<namespace>`: namespace within the registry, for most registries this is the username or organization name.

Example installing package `hello-world` from organization `allexveldman` using `ghcr.io` as the registry:
```commandline
pip install --index-url="https://$GITHUB_USER:$GITHUB_TOKEN@pyoci.com/ghcr.io/allexveldman/" hello-world
```
> [!Warning]
> If the package contains dependencies from regular pypi, these will not resolve.
>
> Pip does not have a proper way of indicating you only want to resolve `<package-name>` through PyOCI and it's dependencies through pypi.
> Poetry does provide you with [a way](https://python-poetry.org/docs/repositories/#package-source-constraint) to do this.

For more examples, including how to publish a package, see the [examples](/docs/examples).

## Host your own
If you don't want, or can't, use https://pyoci.com, you can host your own using the docker container.

`docker run ghcr.io/allexveldman/pyoci:latest`

Note that only HTTP is support at this moment,
PyOCI is expected to run behind a reverse proxy that handles TLS termination, or a trusted environment.

### Environment variables
- `PORT`: port to listen on, defaults to `8080`.
- `PYOCI_PATH`: Host PyOCI on a subpath, for example: `PYOCI_PATH="/acme-corp"`.
- `OTLP_ENDPOINT`: If set, forward logs, traces, and metrics to this OTLP collector endpoint every 30s.
- `OTLP_AUTH`: Full Authorization header value to use when sending OTLP requests.
- `RUST_LOG`: Log filter, defaults to `info`.

The following environment variables will be added as attributes to the OTLP resources:
- `DEPLOYMENT_ENVIRONMENT` -> `deployment.environment`

Set by Azure Container App, can change if I every decide to move host:
- `CONTAINER_APP_NAME` -> `k8s.container.name`
- `CONTAINER_APP_REVISION` -> `k8s.pod.name`
- `CONTAINER_APP_REPLICA_NAME` -> `k8s.replicaset.name`


## Authentication
Pip's [Basic authentication](https://pip.pypa.io/en/stable/topics/authentication/#basic-http-authentication)
is forwarded as-is to the target registry as part of the [token authentication](https://distribution.github.io/distribution/spec/auth/token/) flow.

## Changing a package
PyOCI will refuse to upload a package file if the package name, version and architecture already exist.
To update an existing file, delete it first and re-publish it.

## Deleting a package
There is no formal specification for deleting python packages, instead you can use the OCI registry provided methods to delete your package.

PyOCI also supports deleting a package file using `DELETE /<registry>/<namespace>/<package-name>/<filename>`, support depends on the
underlying registry's support for the [content management](https://github.com/opencontainers/distribution-spec/blob/main/spec.md#content-management)
section of the OCI Distribution specification.

## Renovate + ghcr.io
As PyOCI acts as a private pypi index, Renovate needs to be configured to use credentials for your private packages.
(https://docs.renovatebot.com/getting-started/private-packages/)
To prevent having to check-in [encrypted secrets](https://docs.renovatebot.com/getting-started/private-packages/#encrypting-secrets)
you can:
1. Self-host renovate as a github workflow
2. Set `package: read` permissions for the workflow
3. Pass the `GITHUB_TOKEN` as an environment variable to Renovate
4. Add a hostRule for the Renovate runner to apply basic auth for pyoci using the environment variable
5. In the [package settings](https://docs.github.com/en/packages/learn-github-packages/configuring-a-packages-access-control-and-visibility#ensuring-workflow-access-to-your-package) of the private package give the repository running renovate `read` access.

Note that [at the time of writing](https://github.com/orgs/community/discussions/24636), GitHub App Tokens can't be granted `read:package` permissions,
this is why you'll need to use the `GITHUB_TOKEN`.

`.github/workflows/renovate.yaml`
```yaml
...
concurrency:
  group: Renovate

# Allow the GITHUB_TOKEN to read packages
permissions:
  contents: read
  packages: read

jobs:
  renovate:
    ...
      - name: Self-hosted Renovate
        uses: renovatebot/github-action@v40.2.4
        with:
          configurationFile: config.js
          token: '${{ steps.get_token.outputs.token }}'
        env:
          RENOVATE_PYOCI_USER: pyocibot
          RENOVATE_PYOCI_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`config.js`
```js
module.exports = {
  ...
  hostRules: [
    {
      matchHost: "pyoci.com",
      hostType: "pypi",
      username: process.env.RENOVATE_PYOCI_USER,
      password: process.env.RENOVATE_PYOCI_TOKEN
    },
  ],
};
```
