FROM rust:1.99-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY ui ./ui
RUN cargo build --locked --release -p comicgpu-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libvulkan1 mesa-vulkan-drivers ca-certificates && rm -rf /var/lib/apt/lists/* && useradd -m comic
COPY --from=builder /src/target/release/comicgpu-server /usr/local/bin/comicgpu-server
USER comic
ENV COMICGPU_BIND=0.0.0.0:8501
EXPOSE 8501
CMD ["comicgpu-server"]
