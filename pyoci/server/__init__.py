import base64
import logging
from pathlib import Path
from typing import Annotated, Literal

from fastapi import FastAPI, File, Form, Header
from fastapi import Path as PydanticPath
from fastapi import Request, Response, UploadFile
from fastapi.responses import HTMLResponse, StreamingResponse
from fastapi.templating import Jinja2Templates
from httpx import HTTPStatusError

import pyoci
from pyoci.oci import PackageInfo
from pyoci.oci.client import AuthenticationError
from pyoci.oci.package import FILE_PATTERN

app = FastAPI()
templates = Jinja2Templates(directory=Path(__file__).parent / "templates")
logger = logging.getLogger(__name__)


def parse_auth_header(authorization: str) -> tuple[bytes, bytes]:
    """Parse the Authorization header into username and password."""
    username, password = base64.b64decode(
        authorization.removeprefix("Basic ").encode("utf-8")
    ).split(b":")
    return username, password


@app.post("/{repository}/{namespace}/", name="publish")
async def publish_package(
    repository: str,
    namespace: str,
    action: Annotated[Literal["file_upload"], Form(alias=":action")],
    content: Annotated[UploadFile, File()],
    authorization: Annotated[str | None, Header()] = None,
    pyoci_insecure: Annotated[bool, Header(alias="X-PyOCI-Insecure")] = False,
):
    scheme = "https" if not pyoci_insecure else "http"
    username = password = None
    if authorization is not None:
        username, password = parse_auth_header(authorization)
    with pyoci.oci.Client(
        registry_url=f"{scheme}://{repository}", username=username, password=password
    ) as client:
        package = pyoci.oci.PackageInfo.from_string(
            content.filename, namespace=namespace
        )

        logger.info("Publishing '%s' to '%s'", package, repository)

        index = pyoci.oci.Index.pull(
            name=package.name,
            reference=package.version,
            artifact_type=pyoci.oci.ARTIFACT_TYPE,
            client=client,
        )
        manifest = pyoci.oci.Manifest(
            artifactType=pyoci.oci.ARTIFACT_TYPE,
            config=pyoci.oci.EmptyConfig(),
        )
        manifest.layers.append(
            pyoci.oci.Layer.from_file(
                file=content.file,
                artifact_type=pyoci.oci.ARTIFACT_TYPE,
            )
        )
        index.add_manifest(
            manifest,
            platform=package.platform(),
        )
        index.push(client=client)


@app.get(
    "/{repository}/{namespace}/{package_name}/",
    response_class=HTMLResponse,
    name="list",
)
def list_package(
    repository: str,
    namespace: str,
    package_name: str,
    request: Request,
    response: Response,
    authorization: Annotated[str | None, Header()] = None,
    pyoci_insecure: Annotated[bool, Header(alias="X-PyOCI-Insecure")] = False,
):
    package = PackageInfo(package_name, namespace=namespace)
    username = password = None
    if authorization is not None:
        username, password = parse_auth_header(authorization)
    scheme = "https" if not pyoci_insecure else "http"
    with pyoci.oci.Client(
        registry_url=f"{scheme}://{repository}", username=username, password=password
    ) as client:
        try:
            files = pyoci.oci.list_package(package=package, client=client)
        except HTTPStatusError as e:
            response.status_code = e.response.status_code
            return
        except AuthenticationError:
            response.status_code = 401
            return "Unauthorized"
        return templates.TemplateResponse(
            "list-package.html",
            {
                "request": request,
                "repository": repository,
                "namespace": namespace,
                "package": package.distribution,
                "files": files,
            },
        )


@app.get("/{repository}/{namespace}/{package_name}/{filename}", name="download")
def download_package(
    repository: str,
    namespace: str,
    package_name: str,
    filename: Annotated[str, PydanticPath(pattern=FILE_PATTERN)],
    response: Response,
    authorization: Annotated[str | None, Header()] = None,
    pyoci_insecure: Annotated[bool, Header(alias="X-PyOCI-Insecure")] = False,
):
    package = PackageInfo.from_string(filename, namespace=namespace)
    username = password = None
    if authorization is not None:
        username, password = parse_auth_header(authorization)
    scheme = "https" if not pyoci_insecure else "http"
    with pyoci.oci.Client(
        registry_url=f"{scheme}://{repository}", username=username, password=password
    ) as client:
        try:
            data = pyoci.oci.pull_package(package=package, client=client)
        except HTTPStatusError as e:
            response.status_code = e.response.status_code
            return
        except AuthenticationError:
            response.status_code = 401
            return "Unauthorized"

        def data_iterator():
            yield data

        return StreamingResponse(
            content=data_iterator(),
            status_code=200,
            headers={"Content-Disposition": f'attachment; filename="{filename}"'},
        )
