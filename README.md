# PyOCI
Publish and download (private) python packages using an OCI registry for storage.

[![Test](https://github.com/AllexVeldman/pyoci/actions/workflows/test.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/test.yaml)
[![Examples](https://github.com/AllexVeldman/pyoci/actions/workflows/examples.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/examples.yaml)
[![Deploy](https://github.com/AllexVeldman/pyoci/actions/workflows/deploy.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/deploy.yaml)
[![codecov](https://codecov.io/github/AllexVeldman/pyoci/graph/badge.svg?token=SXFCPX8M22)](https://codecov.io/github/AllexVeldman/pyoci)

## Why PyOCI
As part of my job we create private python packages used in the main application.
To not have to rely on yet-another-cloud-provider, instead I built PyOCI, making `ghcr.io` act like a python index.
This also completely removed the need for separate access management as now GitHub Packages access control applies.

Most subscriptions with cloud providers include an [OCI](https://opencontainers.org/) (docker image) registry where private containers can be published and distributed from.

PyOCI allows using any (private) OCI registry as a python package index, as long as it implements the [OCI distribution specification](https://github.com/opencontainers/distribution-spec/blob/main/spec.md).
It acts as a proxy between pip and the OCI registry.

An instance of PyOCI is available at https://pyoci.allexveldman.nl, to use this proxy, please see the [Examples](#Examples).

Tested registries:
- [ghcr.io](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)


Published packages will show up in the OCI registry UI:

<img width="500" alt="ghcr.io hello-world package versions" src="https://github.com/user-attachments/assets/c3595da9-91e7-4ee6-b890-2ed9baca3c9d">


Distinct distributions will show up as separate architectures for the same version:

<img width="500" alt="ghcr.io hello-world distinct architectures" src="https://github.com/user-attachments/assets/63d130cf-5551-4131-b48b-a6e8f259cbc5">

## Authentication
Basic authentication is forwarded to the target registry.

Currently only Basic authentication is supported.
This is due to pip [only supporting basic authentication](https://pip.pypa.io/en/stable/topics/authentication/#basic-http-authentication)
and [not all OCI registries supporting OAuth](https://distribution.github.io/distribution/spec/auth/oauth/),
instead the [token authentication](https://distribution.github.io/distribution/spec/auth/token/) is used.

## Getting started
To let pip resolve to our private registry, we need to supply some into into the `--extra-index-url`:
- URL of the OCI registry to use.
- namespace within the registry, for most registries this is the username or organization name.

### Examples
To install a package with pip using PyOCI:
```commandline
pip install --extra-index-url="http://<username>:<password>@<pyoci url>/<OCI registry url>/<namespace>/" <package name>
```
Example installing package `bar` from organization `Foo` using `ghcr.io` as the registry:
```commandline
pip install --extra-index-url="https://Foo:$GH_TOKEN@pyoci.allexveldman.nl/ghcr.io/foo/" bar
```

For more examples, including how to publish a package, see the [examples](/docs/examples)

## Changing a package
PyOCI will refuse to upload a package file if the package name, version and architecture already exist.
To update an existing file, delete it first and re-publish it.

## Deleting a package
PyOCI does not provide a way to delete a package, instead you can use the OCI registry provided methods to delete your package.

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
      matchHost: "pyoci.allexveldman.nl",
      hostType: "pypi",
      username: process.env.RENOVATE_PYOCI_USER,
      password: process.env.RENOVATE_PYOCI_TOKEN
    },
  ],
};
```
