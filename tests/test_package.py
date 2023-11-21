from pathlib import Path

import pytest

from pyoci.oci.package import PackageInfo


@pytest.mark.parametrize(
    "path,expected,exp_version",
    [
        (
            Path(
                "pyoci_example-2.5.1.dev4+g1664eb2.d20231017-cp311-cp311-macosx_13_0_x86_64.whl"
            ),
            PackageInfo(
                distribution="pyoci_example",
                full_version="2.5.1.dev4+g1664eb2.d20231017",
                architecture="cp311-cp311-macosx_13_0_x86_64.whl",
            ),
            "2.5.1.dev4-g1664eb2.d20231017",
        ),
        (
            Path("pyoci_example-2.5.1-cp311-cp311-macosx_13_0_x86_64.whl"),
            PackageInfo(
                distribution="pyoci_example",
                full_version="2.5.1",
                architecture="cp311-cp311-macosx_13_0_x86_64.whl",
            ),
            "2.5.1",
        ),
        (
            Path("pyoci-0.1.0.tar.gz"),
            PackageInfo(
                distribution="pyoci", full_version="0.1.0", architecture=".tar.gz"
            ),
            "0.1.0",
        ),
        (
            Path("foo") / "pyoci-0.1.0.tar.gz",
            PackageInfo(
                distribution="pyoci", full_version="0.1.0", architecture=".tar.gz"
            ),
            "0.1.0",
        ),
    ],
)
def test_package_info_from_path(path, expected, exp_version):
    assert (pi := PackageInfo.from_path(path)) == expected
    assert pi.version == exp_version
    assert str(path.name) == str(pi)
