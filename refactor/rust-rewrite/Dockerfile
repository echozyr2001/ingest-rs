# Multi-stage build for Rust application
FROM rust:1.75-alpine AS builder

# Install system dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

# Set the working directory
WORKDIR /app

# Copy Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Copy source code
COPY src/ ./src/
COPY tests/ ./tests/

# Build the application in release mode
RUN cargo build --release

# Runtime stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

# Create a non-root user
RUN addgroup -g 1000 inngest && \
    adduser -D -s /bin/sh -u 1000 -G inngest inngest

# Set the working directory
WORKDIR /home/inngest

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/inngest ./inngest

# Change ownership to the inngest user
RUN chown inngest:inngest ./inngest

# Switch to the inngest user
USER inngest

# Expose the port
EXPOSE 8288

# Set default command
CMD ["./inngest", "dev", "--host", "0.0.0.0"]
