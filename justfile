# Examples showcasing PyOci
mod examples 'docs/examples'

[group("dev")]
cf-worker *args:
    NO_MINIFY=1 npx wrangler dev --port 8090 --local-upstream localhost:8090 {{args}}

# Build the WASM module for the cloudflare worker environment
[group("dev")]
build *args:
    rm -rf ./build/
    wasm-pack build --no-typescript --target bundler --out-dir "build" --out-name "pyoci" {{args}}
    cp -f ./src/js/pyoci.js ./build/
    cp ./src/js/cf_worker.js ./build/
    cd ./build && npx esbuild --external:./pyoci_bg.wasm --external:cloudflare:sockets --external:cloudflare:workers --format=esm --bundle ./cf_worker.js --outfile=cf_worker.mjs --minify

# Recreate the OCI registry, clears it's package cache
[group("setup")]
refresh-registry:
    docker compose -f docker/docker-compose.yaml up --force-recreate --wait registry

# Run a smoketest on a local pyoci instance and a local OCI registry
[group("test")]
test-smoke: refresh-registry
    just examples::poetry-publish "0.1.0+1234" "" "" "http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/pyoci/"
    just examples::poetry-install "0.1.0+1234" "" "" "http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/pyoci/"
