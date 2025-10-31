# syntax=docker/dockerfile:1

ARG BUILDKIT_SBOM_SCAN_CONTEXT=true
ARG BUILDKIT_SBOM_SCAN_STAGE=true

FROM rust:1.91.0 AS builder

COPY docker/target-platform.sh /target-platform.sh

ARG TARGETPLATFORM

RUN rustup target add $(/target-platform.sh)
RUN apt update \
    && apt install --no-install-recommends -y \
    musl-tools \
    musl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -u 10001 pyoci

WORKDIR /pyoci

# Build the dependencies as a separate step so they get cached.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs
RUN cargo build --release --target $(/target-platform.sh)

# Copy the actual sources
COPY src ./src/
RUN touch -a -m ./src/main.rs
RUN cargo build --release --target $(/target-platform.sh) && \
    mv /pyoci/target/$(/target-platform.sh)/release/pyoci /pyoci/target/pyoci


FROM scratch

COPY --from=builder /etc/passwd /etc/passwd

ENV PORT=8080
EXPOSE $PORT

# Copy the binary from the builder stage
WORKDIR /pyoci
COPY LICENSE LICENCE
COPY templates ./templates/
COPY --from=builder /pyoci/target/pyoci pyoci

USER pyoci

CMD ["./pyoci"]
