cli-list:
    cargo run -- -u AllexVeldman -p $GH_TOKEN list ghcr.io/allexveldman/pyoci

cf-worker *args:
    NO_MINIFY=1 npx wrangler dev --port 8090 --local-upstream localhost:8090 {{args}}
