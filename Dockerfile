FROM rustlang/rust:nightly AS builder
WORKDIR app

RUN cargo init --name dummy
COPY Cargo.toml .
COPY Cargo.lock .
COPY build.rs .
RUN cargo build --release
COPY src/ src/
# commit id of build etc.
COPY .git/ .git/
# force rebuild main.rs
RUN touch src/main.rs
RUN cargo build --release

FROM debian:buster-slim AS runtime
WORKDIR app
COPY --from=builder /app/target/release/bclicker-server /usr/local/bin
COPY filehost/ filehost/

CMD ["/usr/local/bin/bclicker-server"]
EXPOSE 2137
