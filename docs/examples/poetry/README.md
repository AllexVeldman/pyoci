PyOCI Poetry
============

This is a sample project to demonstrate how to publish a package to a OCI compliant registry using PyOci.
This example uses [poetry](https://python-poetry.org/).

To publish the package:
```bash
poetry build
poetry publish -r pyoci-ghcr-allexveldman [-u <username> -p <password>]
```

`pyoci-ghcr-allexveldman` is defined in [poetry.toml](poetry.toml)

When published successfully, the package should show up in the registry with 2 architectures, `any/.tar.gz` and `any/py3-none-any.whl`.
