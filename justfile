[group("cli")]
cli-list:
    cargo run -- -u AllexVeldman -p $GH_TOKEN list ghcr.io/allexveldman/pyoci

[group("dev")]
cf-worker *args:
    NO_MINIFY=1 npx wrangler dev --port 8090 --local-upstream localhost:8090 {{args}}

[group("dev")]
build *args:
    rm -rf ./build/
    wasm-pack build --no-typescript --target bundler --out-dir "build" --out-name "pyoci" {{args}}
    cp -f ./src/js/pyoci.js ./build/
    cp ./src/js/cf_worker.js ./build/
    cd ./build && npx esbuild --external:./pyoci_bg.wasm --external:cloudflare:sockets --external:cloudflare:workers --format=esm --bundle ./cf_worker.js --outfile=cf_worker.mjs --minify

[group("curl")]
local-publish: refresh-registry
    poetry version -C tests/hello-world 0.1.0
    cd tests/hello-world && poetry build
    curl -v http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/allexveldman/ \
    -F ":action=file_upload" \
    -F protocol_version=1 \
    -F filetype=sdist \
    -F pyversion=source \
    -F metadata_version=2.3 \
    -F name=hello_world \
    -F version=0.1.0 \
    -F content=@tests/hello-world/dist/hello_world-0.1.0.tar.gz

[group("curl")]
local-list:
    curl -v http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/allexveldman/hello-world/

[group("curl")]
local-download:
    curl -vOJ http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/allexveldman/hello_world/hello_world-0.1.0.tar.gz


[group("setup")]
refresh-registry:
    docker compose -f docker/docker-compose.yaml up --force-recreate --wait registry

[group("poetry")]
poetry-local-publish version: refresh-registry
    rm -rf tests/hello-world/dist
    poetry version -C tests/hello-world {{version}}
    cd tests/hello-world && poetry build
    poetry publish -C tests/hello-world -r pyoci-local

[group("poetry")]
poetry-local-install version:
    poetry add -C tests/dep-test hello-world@{{version}} --source pyoci-local --no-cache

[group("test")]
test-smoke: (poetry-local-publish "0.1.0+1234") (poetry-local-install "0.1.0+1234")
    poetry run -C tests/dep-test python -m hello_world
