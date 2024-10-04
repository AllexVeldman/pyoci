FROM rust:1.81.0 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev

WORKDIR /app

# Build the dependencies as a separate step so they get cached.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl

# Copy the actual sources
COPY templates ./templates/
COPY src ./src/
RUN touch -a -m ./src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM scratch

ENV PORT=8080
EXPOSE $PORT

# Copy the binary from the builder stage
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/pyoci pyoci

CMD ["./pyoci"]
