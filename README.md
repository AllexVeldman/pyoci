# PyOCI
Publish and download python packages using OCI registries.

[![Test](https://github.com/AllexVeldman/pyoci/actions/workflows/test.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/test.yaml)
[![Examples](https://github.com/AllexVeldman/pyoci/actions/workflows/examples.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/examples.yaml)
[![Deploy](https://github.com/AllexVeldman/pyoci/actions/workflows/deploy.yaml/badge.svg)](https://github.com/AllexVeldman/pyoci/actions/workflows/deploy.yaml)

PyOCI allows using any (private) OCI registry as a python package index.
It acts as a proxy between pip and the OCI registry.

Basic authentication is forwarded to the target registry.

For PyOCI to resolve to the correct package, the following parts are needed as part of the index-url:
- OCI registry url, https is assumed
- namespace, for most registries this is the username or organization name
- name of the python package

Currently only Basic authentication is supported.
This is due to pip [only supporting basic authentication](https://pip.pypa.io/en/stable/topics/authentication/#basic-http-authentication)
and [not all OCI registries supporting OAuth](https://distribution.github.io/distribution/spec/auth/oauth/),
instead the [token authentication](https://distribution.github.io/distribution/spec/auth/token/) is used.

To install a package with pip using PyOCI:
```commandline
pip install --extra-index-url=http://<username>:<password>@<pyoci url>/<OCI registry url>/<namespace>/<package name>
```
Example installing package `bar` from user `Foo` using `ghcr.io` as the registry:
```commandline
pip install --extra-index-url=https://Foo:$GH_TOKEN@pyoci.allexveldman.nl/ghcr.io/foo/bar
```

For more examples, see the [examples](/docs/examples)

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
