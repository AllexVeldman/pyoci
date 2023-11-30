"""
PyOCI integration tests.
Both a PyOCI server and an OCI registry should be running
for any of these tests to work.

Note that none of these tests use Autorization headers.
Also, none of these tests should be run in parallel.
"""
import httpx
import pytest

PYOCI_HOST = "http://localhost:8084"
OCI_HOST = "localhost:5000"


@pytest.mark.parametrize(
    "package",
    [
        "test_package-0.1.0.tar.gz",
        "test_package-0.1.0-py3-none-any.whl",
        "test_package-0.1.0.dev4+g1664eb2.d20231017-cp311-cp311-macosx_13_0_x86_64.whl",
    ],
)
def test_publish(testdata, package):
    """Test if we can publish a package version"""
    with (testdata / package).open("rb") as file:
        response = httpx.post(
            f"{PYOCI_HOST}/{OCI_HOST}/test/",
            headers={"X-PyOCI-Insecure": "true"},
            data={":action": "file_upload"},
            files={"content": file},
        )
    response.raise_for_status()


@pytest.mark.parametrize("name", ["test_package", "test-package"])
def test_list(testdata, name):
    """Test if we can list package versions"""
    # Ensure the package exists
    with (testdata / "test_package-0.1.0.tar.gz").open("rb") as file:
        response = httpx.post(
            f"{PYOCI_HOST}/{OCI_HOST}/test/",
            headers={"X-PyOCI-Insecure": "true"},
            data={":action": "file_upload"},
            files={"content": file},
        )
    response.raise_for_status()

    # List the package versions
    response = httpx.get(
        f"{PYOCI_HOST}/{OCI_HOST}/test/{name}/",
        headers={"X-PyOCI-Insecure": "true"},
    )
    response.raise_for_status()
    assert (
        f'<a href="{PYOCI_HOST}/{OCI_HOST}'
        f'/test/test-package/test_package-0.1.0.tar.gz">'
        f"test_package-0.1.0.tar.gz</a>"
    ) in response.read().decode("utf-8")


@pytest.mark.parametrize("name", ["test_package", "test-package"])
@pytest.mark.parametrize(
    "filename",
    [
        "test_package-0.1.0.tar.gz",
        "test_package-0.1.0-py3-none-any.whl",
        "test_package-0.1.0.dev4+g1664eb2.d20231017-cp311-cp311-macosx_13_0_x86_64.whl",
    ],
)
def test_download(testdata, name, filename):
    """Test if we can list package versions"""
    # Ensure the package exists
    test_file = testdata / filename
    with test_file.open("rb") as file:
        response = httpx.post(
            f"{PYOCI_HOST}/{OCI_HOST}/test/",
            headers={"X-PyOCI-Insecure": "true"},
            data={":action": "file_upload"},
            files={"content": file},
        )
    response.raise_for_status()

    # Download the package
    response = httpx.get(
        f"{PYOCI_HOST}/{OCI_HOST}/test/{name}/{filename}",
        headers={"X-PyOCI-Insecure": "true"},
    )
    response.raise_for_status()
    assert response.read() == test_file.read_bytes()
