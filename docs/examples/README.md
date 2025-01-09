# Examples

The examples in this directory serve both as a reference on how to use PyOCI as well as a way to run these examples.

## Running the examples

Examples can be run using [just](https://github.com/casey/just) either from this directory (`just poetry-install`)
or from any other directory in the repository (`just examples::poetry-install`).

`just --list --list-submodules` to list all options.

By default all examples require at least a `username` and `token`, assume a PyOCI instance is running on `localhost:8080`,
and you have `read` (and `write` for publish) access to [ghcr.io/allexveldman/hello_world](https://github.com/AllexVeldman/pyoci/pkgs/container/hello_world).

Most will only have `read` access, to run the publish examples you will have to target your own namespace: `just poetry-publish 0.0.1+1234 "$GITHUB_USER" "$GITHUB_TOKEN" "https://localhost:8080/ghcr.io/acme-corp/"`.\
This will publish a `hello_world` package version `0.0.1+1234` to the `acme-corp` organization.

You can replace `localhost:8080` with `https://pyoci.com` or your own deployment URL to use that instance instead.
