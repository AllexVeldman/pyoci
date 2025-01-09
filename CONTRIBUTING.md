## How to contribute to PyOCI

The most appreciated contributions in the current phase of this project are
feature requests and bug reports.

PyOCI is a small project so I can't guarantee PR's without an issue will be merged.

### Development

To build and run PyOCI, run `cargo run`, this will start the server at 0.0.0.0:8080.\
To run the tests, run `cargo test`.

Examples can be run using [just](https://github.com/casey/just), for more information see the [examples](docs/examples).

#### Code style
[pre-commit](https://pre-commit.com/) is used to check for style issues.

Before committing any code, run `pre-commit install` to install the git hooks.\
This will ensure you run `cargo fmt` and `cargo clippy -- -D warnings` before committing changes.
