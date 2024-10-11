# Examples showcasing PyOci
mod examples 'docs/examples'

# Run tests with coverage, requires cargo-llvm-cov. Add `--open` to open the report in the browser.
[group("ci")]
test-coverage *args:
    cargo llvm-cov --lcov --output-path lcov.info
    cargo llvm-cov report {{args}}

# Recreate the OCI registry, clears it's package cache
[group("setup")]
refresh-registry:
    docker compose -f docker/docker-compose.yaml up --build --force-recreate --wait pyoci registry

# Run a smoketest on a local pyoci instance and a local OCI registry
[group("ci")]
test-smoke: refresh-registry
    just examples::poetry-publish "0.1.0+1234" "foo" "bar" "http://localhost:8080/http%3A%2F%2Fregistry%3A5000/pyoci/"
    just examples::poetry-install "0.1.0+1234" "foo" "bar" "http://localhost:8080/http%3A%2F%2Fregistry%3A5000/pyoci/"
    just examples::curl-list-json "foo" "bar" "http://localhost:8080/http%3A%2F%2Fregistry%3A5000/pyoci/"
    just examples::curl-delete "0.1.0+1234" "foo" "bar" "http://localhost:8080/http%3A%2F%2Fregistry%3A5000/pyoci/"
