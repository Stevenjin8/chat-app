FROM rust:latest AS builder
WORKDIR /usr/src/chat-app
RUN cargo init .
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build
RUN rm -rf ./target/debug/deps/hello*

COPY ./src ./src
RUN cat ./src/main.rs
RUN cargo build

FROM ubuntu:latest

WORKDIR /root
COPY --from=0 /usr/src/chat-app/target/debug/hello  ./
EXPOSE 8081
CMD ["./hello"]
