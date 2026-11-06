FROM rust:1-slim AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p mahjongi-server

FROM debian:stable-slim
WORKDIR /app
COPY --from=build /app/target/release/mahjongi-server /usr/local/bin/mahjongi-server
COPY web ./web
ENV PORT=8080 WEB_DIR=/app/web
EXPOSE 8080
CMD ["mahjongi-server"]
