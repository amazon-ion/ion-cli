FROM rust:1.64-slim-buster as builder
WORKDIR /usr/src/ion-cli
COPY . .
RUN cargo install --verbose --path .

FROM debian:11.1-slim
COPY --from=builder /usr/local/cargo/bin/ion /usr/bin/ion
CMD /usr/bin/ion
VOLUME /data
