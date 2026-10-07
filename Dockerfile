# ═══════════════════════════════════════════════════════════════
# Stage 1: Backend (Rust)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/rust:1.98.1-alpine3.21 AS backend-builder

RUN apk add --no-cache --update \
    build-base \
    musl-dev \
    pkgconfig

WORKDIR /build

# sqlite-vec.c declares `typedef u_intN_t ...` without including
# <sys/types.h>. On glibc this goes unnoticed because <stdint.h> pulls in
# <bits/types.h>, which declares u_intN_t under __USE_MISC; on musl it does
# not. Alias the types to their <stdint.h> equivalents instead of forcing a
# `-include`, because CFLAGS is global and also reaches the .S files of
# aws-lc-sys. There a `-include` injects C headers into the assembler;
# a `-D` only defines a preprocessor macro.
ENV CFLAGS="-Du_int8_t=uint8_t -Du_int16_t=uint16_t -Du_int64_t=uint64_t"

# Cache dependencies (avoid recompiling every time)
RUN cargo init --bin --name valet . && \
    echo "pub fn dummy() {}" > src/lib.rs && \
    mkdir -p src/bin && \
    echo "fn main() {}" > src/bin/seed.rs && \
    echo "fn main() {}" > src/bin/reindex.rs

COPY Cargo.toml Cargo.lock ./
RUN cargo build --release && \
    rm -rf src

COPY src ./src
RUN touch src/main.rs src/lib.rs && \
    cargo build --release && \
    strip target/release/valet

# ═══════════════════════════════════════════════════════════════
# Stage 2: Frontend (Node)
# ═══════════════════════════════════════════════════════════════
FROM docker.io/library/node:22-alpine AS frontend-builder

WORKDIR /build
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend/ ./
ENV CI=true
RUN npm run build

# `npm run build` genera `dist/.vite/manifest.json` (lo consume
# `frontend/scripts/check-initial-bundle.mjs` durante el propio build). No debe
# servirse en producción (el backend sirve `static` con ServeDir), así que se
# elimina antes de copiar la imagen final.
RUN rm -rf dist/.vite

# ═══════════════════════════════════════════════════════════════
# Stage 3: Runtime
# ═══════════════════════════════════════════════════════════════
FROM alpine:3.21

ENV RUST_LOG=info

RUN apk add --no-cache --update \
    ca-certificates && \
    rm -rf /var/cache/apk/*

WORKDIR /app
COPY --from=backend-builder /build/target/release/valet /app/valet
COPY --from=backend-builder /build/target/release/valet-reindex /app/valet-reindex
COPY --from=frontend-builder /build/dist /app/static
COPY migrations ./migrations

EXPOSE 3000
CMD ["/app/valet"]
