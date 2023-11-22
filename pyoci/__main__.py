import logging
from pathlib import Path

import click
import uvicorn

import pyoci


@click.group()
def cli():
    pass


class OCI:
    def __init__(
        self,
        registry: str,
        username: str | None = None,
        password: str | None = None,
        debug: bool = False,
    ):
        if debug:
            logging.basicConfig(level=logging.DEBUG)
        self.client = pyoci.oci.Client(
            registry_url=registry, username=username, password=password
        )


@cli.group()
@click.option("-r", "--registry", help="Registry URL", required=True)
@click.option("-u", "--username", help="Username", default=None)
@click.option("-p", "--password", help="Password", default=None)
@click.option("-d", "--debug", help="Debug output", is_flag=True)
@click.pass_context
def oci(ctx, registry, username, password, debug):
    ctx.obj = OCI(registry=registry, username=username, password=password, debug=debug)


@oci.command()
@click.argument("path", type=click.Path(exists=True, path_type=Path))
@click.option("--namespace", help="Namespace", default="")
@click.pass_context
def publish(ctx, path: Path, namespace: str):
    """Publish a directory of packages to the registry."""
    obj: OCI = ctx.ensure_object(OCI)
    with obj.client as client:
        for package in path.iterdir():
            pyoci.oci.publish_package(path=package, client=client, namespace=namespace)


@oci.command()
@click.argument("name")
@click.option("--namespace", help="Namespace", default="")
@click.pass_context
def list(ctx, name: str, namespace: str):
    """List all versions of a package in the registry."""
    obj: OCI = ctx.ensure_object(OCI)
    with obj.client as client:
        for p in pyoci.oci.list_package(name=name, client=client, namespace=namespace):
            print(p)


@oci.command()
@click.argument("package")
@click.option("--namespace", help="Namespace", default="")
@click.option(
    "--output",
    help="Output directory",
    default="out",
    type=click.Path(
        exists=True,
        path_type=Path,
        file_okay=False,
        dir_okay=True,
    ),
)
@click.pass_context
def pull(ctx, package: str, namespace: str = "", output: str = "out"):
    """Download a package from the registry."""
    obj: OCI = ctx.ensure_object(OCI)
    with obj.client as client:
        data = pyoci.oci.pull_package(
            package=package, client=client, namespace=namespace
        )
        destination = Path(output) / package
        destination.write_bytes(data)
        print(f"Done downloading: {destination}")


LOGGING_CONFIG = {
    "version": 1,
    "disable_existing_loggers": False,
    "formatters": {
        "default": {
            "()": "uvicorn.logging.DefaultFormatter",
            "fmt": "%(levelprefix)s %(message)s",
            "use_colors": None,
        },
        "access": {
            "()": "uvicorn.logging.AccessFormatter",
            "fmt": '%(levelprefix)s %(client_addr)s - "%(request_line)s" %(status_code)s',  # noqa: E501
        },
    },
    "handlers": {
        "default": {
            "formatter": "default",
            "class": "logging.StreamHandler",
            "stream": "ext://sys.stderr",
        },
        "access": {
            "formatter": "access",
            "class": "logging.StreamHandler",
            "stream": "ext://sys.stdout",
        },
    },
    "loggers": {
        "uvicorn": {"handlers": ["default"], "level": "INFO", "propagate": False},
        "uvicorn.error": {"level": "INFO"},
        "uvicorn.access": {"handlers": ["access"], "level": "INFO", "propagate": False},
        "pyoci": {"handlers": ["default"], "level": "INFO", "propagate": False},
    },
}


@cli.command()
@click.option("--reload", help="Watch for changes", is_flag=True)
@click.option("-p", "--port", type=int, default=8080)
def server(reload: bool = False, port: int = 8080):
    uvicorn.run(
        "pyoci.server:app",
        port=port,
        log_level="info",
        log_config=LOGGING_CONFIG,
        reload=reload,
    )


if __name__ == "__main__":
    cli()


"""
package:
    - index-url
    - name
    - version
    - 1 or more files:
        - content
        - architecture (filename)

OCI artifact (index):
    - registry (index-url)
    - namespace (does not exist in the python package, use path param from index-url)
    - name (package name)
    - reference (package version)
    - 1 or more manifests: (1 per package architecture)
        - digest
        - architecture
        - os
        - 1 or more blobs: (we only use 1 for the actual package)
            - content
"""
