FROM rust:latest AS build
WORKDIR /app
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./src ./src
RUN cargo build --release

FROM debian:stable
COPY --from=build /app/target/release/kafka-demo .
CMD ["./kafka-demo"]
