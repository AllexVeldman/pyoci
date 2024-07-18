PyOCI Poetry
============

This is a sample project to demonstrate how to publish/install a package to an OCI compliant registry using PyOci.
This example uses [poetry](https://python-poetry.org/).

To publish the package:
```bash
poetry build
poetry publish -r pyoci-ghcr-allexveldman [-u <username> -p <password>]
```

`pyoci-ghcr-allexveldman` is defined in [poetry.toml](publish/poetry.toml)

When published successfully, the package should show up in the registry with 2 architectures, `any/.tar.gz` and `any/py3-none-any.whl`.

To install the package:
```bash
poetry source add --priority=explicit pyoci https://pyoci.allexveldman.nl/ghcr.io/allexveldman/
poetry config http-basic.pyoci <username> <password>
poetry add hello-world --source pyoci-ghcr-allexveldman
```

Note that we use `--priority=explicit` so pyoci is only queried for packages explicitly added from that source.
