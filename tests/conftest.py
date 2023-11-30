from pathlib import Path

import pytest

TEST_DATA = Path(__file__).parent / "testdata"


@pytest.fixture
def testdata() -> Path:
    """Return the testdata dir for this module"""
    return TEST_DATA
