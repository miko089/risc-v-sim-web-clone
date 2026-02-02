FROM krinkin/rv64-toolchain:latest

# Install dependencies and Rust
RUN apt-get update && apt-get install -y curl pkg-config libssl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    apt-get clean && rm -rf /var/lib/apt/lists/*
ENV PATH="/root/.cargo/bin:$PATH"

COPY . /app

WORKDIR /app/risc-v-sim
RUN stat Cargo.toml
RUN cargo build --release
RUN cp target/release/risc-v-sim /usr/local/bin/
RUN risc-v-sim --help

# Build risc-v-sim-web
WORKDIR /app
RUN cargo build --release

# Set default environment variables (can be overridden at runtime)
ENV SIMULATOR_BINARY="/usr/local/bin/risc-v-sim"
ENV AS_BINARY="riscv64-linux-gnu-as"
ENV LD_BINARY="riscv64-linux-gnu-ld"
ENV CODESIZE_MAX="2048"
ENV TICKS_MAX="128"
ENV MONGODB_URI="mongodb://host.docker.internal:27017"
ENV MONGODB_DB="riscv_sim"
ENV SUBMISSIONS_FOLDER="submission"

# GitHub OAuth - Set these at runtime with -e flags
ENV GITHUB_CLIENT_ID=""
ENV GITHUB_CLIENT_SECRET=""

# JWT Secret - Set at runtime with -e flag
ENV JWT_SECRET=""

# Create submissions directory
RUN mkdir -p /app/submission

EXPOSE 3000

ENTRYPOINT ["./target/release/risc-v-sim-web"]
