FROM rust:1.56-slim as builder
ENV builddeps="cmake git gcc g++ clang"
WORKDIR /usr/src/ion-cli
COPY . .
RUN apt-get update -y \
  && apt-get install -y ${builddeps} \
  && git submodule update --init --recursive
RUN cargo install --path .

FROM debian:11.1-slim
COPY --from=builder /usr/local/cargo/bin/ion /usr/bin/ion
CMD /usr/bin/ion
VOLUME /data
