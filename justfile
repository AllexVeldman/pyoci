cli-list:
    cargo run -- -u AllexVeldman -p $GH_TOKEN list ghcr.io/allexveldman/pyoci

cf-worker *args:
    NO_MINIFY=1 npx wrangler dev --port 8090 --local-upstream localhost:8090 {{args}}

local-publish:
    curl -v http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/allexveldman \
    -F ":action=file_upload" \
    -F protocol_version=1 \
    -F filetype=sdist \
    -F pyversion=source \
    -F metadata_version=2.3 \
    -F name=pyoci \
    -F version=0.1.0 \
    -F content=@py/dist/pyoci-0.1.0.tar.gz

local-list:
    curl -v http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/allexveldman/hello-world/

local-download:
    curl -vOJ http://localhost:8090/http%3A%2F%2Flocalhost%3A5000/allexveldman/pyoci/pyoci-0.1.0.tar.gz
