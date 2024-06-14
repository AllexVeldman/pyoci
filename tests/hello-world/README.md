PyOCI Hello World
=================

This is a sample project to demonstrate how to publish a package to a OCI compliant registry using PyOci.
This example uses [poetry](https://python-poetry.org/) but the same applies to other tools like pip and twine.

To publish the hello-world package:
```bash
poetry build
poetry publish -r <repo> [-u <username> -p <password>]
```

When published successfully, the package should show up in the registry with 2 architectures, `any/.tar.gz` and `any/py3-none-any.whl`.

`poetry.toml` defines 3 repositories:
- `pyoci-local` points to a local pyoci instance running on `http://localhost:8090` with a local OCI registry running on `http://localhost:5000`
  This repo is used by `just test-smoke` together with `just cf-worker` and does not require authentication.
- `pyoci-local-ghcr` points to a local pyoci instance running on `http://localhost:8090` in combination with the ghcr.io registry.
- `pyoci-ghcr` points to `https://pyoci.allexveldman.nl` in combination with the ghcr.io registry.

Note that the ghcr.io registry [needs authentication](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-to-the-container-registry) to publish packages.
