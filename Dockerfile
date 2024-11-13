FROM rust:1.80-alpine AS builder
RUN apk add --no-cache build-base
WORKDIR /usr/src/ta-agi-doorbell
COPY . .
RUN cargo build --release
CMD ["ta-agi-doorbell"]

FROM alpine:latest
WORKDIR /ta-agi-doorbell
COPY --from=builder /usr/src/ta-agi-doorbell/target/release/ta-agi-doorbell ./
CMD ["./ta-agi-doorbell"]

