# build stage: rust:alpine targets musl by default, producing a static binary
FROM rust:1-alpine AS builder

RUN apk add --no-cache \
  build-base \
  ca-certificates \
  openssl-dev \
  openssl-libs-static \
  perl

# force openssl-sys to link openssl statically so the final binary has no shared deps
ENV OPENSSL_STATIC=1
ENV OPENSSL_NO_VENDOR=1

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

RUN adduser -D -H -u 10001 geolite

FROM scratch

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /ca-certificates.crt
ENV SSL_CERT_FILE=/ca-certificates.crt

# passwd/group entries let the numeric UID below resolve to the geolite account
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

COPY --from=builder /build/target/release/geolite /geolite

USER geolite

ENTRYPOINT ["/geolite"]
