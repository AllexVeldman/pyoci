import base64
import logging
from pathlib import Path
from typing import Annotated

from fastapi import FastAPI, Header, Request, Response
from fastapi.responses import HTMLResponse, StreamingResponse
from fastapi.templating import Jinja2Templates
from httpx import HTTPStatusError

import pyoci
from pyoci.oci.client import AuthenticationError

app = FastAPI()
templates = Jinja2Templates(directory=Path(__file__).parent / "templates")

logger = logging.getLogger(__name__)


@app.get("/")
def read_root():
    return {"Hello": "World"}


@app.get(
    "/{repository}/{namespace}/{package}/", response_class=HTMLResponse, name="list"
)
def list_package(
    repository: str,
    namespace: str,
    package: str,
    request: Request,
    response: Response,
    authorization: Annotated[str | None, Header()] = None,
):
    username = password = None
    if authorization is not None:
        username, password = base64.b64decode(
            authorization.removeprefix("Basic ").encode("utf-8")
        ).split(b":")
    with pyoci.oci.Client(
        registry_url=f"https://{repository}", username=username, password=password
    ) as client:
        try:
            files = pyoci.oci.list_package(
                name=package, client=client, namespace=namespace
            )
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
                "package": package,
                "files": files,
            },
        )


@app.get(
    "/{repository}/{namespace}/{package}/{filename}",
    name="file",
)
def list_package(
    repository: str,
    namespace: str,
    package: str,
    filename: str,
    response: Response,
    authorization: Annotated[str | None, Header()] = None,
):
    assert filename.startswith(package), "filename must start with package name"
    username = password = None
    if authorization is not None:
        username, password = base64.b64decode(
            authorization.removeprefix("Basic ").encode("utf-8")
        ).split(b":")
    with pyoci.oci.Client(
        registry_url=f"https://{repository}", username=username, password=password
    ) as client:
        try:
            data = pyoci.oci.pull_package(
                package=filename, client=client, namespace=namespace
            )
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
