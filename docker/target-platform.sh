#!/bin/bash

# Convert the TARGETPLATFORM environment variable
# To the rust build target
case $TARGETPLATFORM in
    'linux/amd64')
        echo -n 'x86_64-unknown-linux-musl';
    ;;
    'linux/arm64')
        echo -n 'aarch64-unknown-linux-musl';
    ;;
    *)
        echo "Unsupported TARGETPLATFORM: '$TARGETPLATFORM'" >&2;
        exit 1;
    ;;

esac
