# syntax=docker/dockerfile:1

ARG BUILDKIT_SBOM_SCAN_CONTEXT=true
ARG BUILDKIT_SBOM_SCAN_STAGE=true

FROM rust:1.81.0 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev

RUN useradd -u 10001 pyoci

WORKDIR /pyoci

# Build the dependencies as a separate step so they get cached.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl

# Copy the actual sources
COPY src ./src/
RUN touch -a -m ./src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM scratch

COPY --from=builder /etc/passwd /etc/passwd

ENV PORT=8080
EXPOSE $PORT

# Copy the binary from the builder stage
WORKDIR /pyoci
COPY LICENSE LICENCE
COPY templates ./templates/
COPY --from=builder /pyoci/target/x86_64-unknown-linux-musl/release/pyoci pyoci

USER pyoci

CMD ["./pyoci"]
