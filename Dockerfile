FROM rust:1-alpine AS builder

RUN apk add --no-cache \
  build-base \
  ca-certificates \
  openssl-dev \
  openssl-libs-static \
  perl

ENV OPENSSL_STATIC=1
ENV OPENSSL_NO_VENDOR=1

RUN adduser -D -H -u 10001 geolite && mkdir /seed

ARG VERSION

RUN test -n "$VERSION" || { echo "VERSION build-arg is required"; exit 1; }

RUN cargo install geolite --version "$VERSION" --locked --root /out

FROM scratch

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /ca-certificates.crt
ENV SSL_CERT_FILE=/ca-certificates.crt

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

COPY --from=builder /out/bin/geolite /geolite

COPY --from=builder --chown=10001:10001 /seed /.geolite
ENV HOME=/

USER geolite

ENTRYPOINT ["/geolite"]
