# PyOCI
Publish and download python packages using OCI registries

PyOCI allows using any (private) OCI registry as a python package index.
It acts as a proxy between pip and the OCI registry.

Basic authentication is forwarded to the target registry.

For PyOCI to resolve to the correct package, the following parts are needed as part of the index-url:
- OCI registry url (without schema, HTTPS is assumed since this package is mainly intended for private registries)
- namespace, for most registries this is the username or organization name
- name of the python package

Currently only Basic authentication is supported.
This is due to pip [only supporting basic authentication](https://pip.pypa.io/en/stable/topics/authentication/#basic-http-authentication)
and [not all OCI registries supporting OAuth](https://distribution.github.io/distribution/spec/auth/oauth/),
instead the [token authentication](https://distribution.github.io/distribution/spec/auth/token/) is used.

To install a package with pip using PyOCI:
```commandline
pip install --extra-index-url=http://<username>:<password>@<pyoci url>/<OCI registry url>/<namespace>/ <package name>
```
Example installing package `bar` from user `Foo` using `ghcr.io` as the registry:
```commandline
pip install --extra-index-url=https://Foo:$GH_TOKEN@example.pyoci.com/ghcr.io/foo/ bar
```
