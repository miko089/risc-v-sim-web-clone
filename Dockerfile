FROM krinkin/rv64-toolchain:latest

# Install dependencies and Rust
RUN apt-get update && apt-get install -y curl && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    apt-get clean && rm -rf /var/lib/apt/lists/*
ENV PATH="/root/.cargo/bin:$PATH"

# Build risc-v-sim
WORKDIR /tmp
RUN git clone https://github.com/nup-csai/risc-v-sim.git
WORKDIR /tmp/risc-v-sim
RUN cargo build --release
RUN cp target/release/risc-v-sim /usr/local/bin/
RUN risc-v-sim --help

# Build risc-v-sim-web
WORKDIR /app
COPY . .
RUN cargo build --release

# Set environment variables
ENV SIMULATOR_BINARY="/usr/local/bin/risc-v-sim"
ENV AS_BINARY="riscv64-linux-gnu-as"
ENV LD_BINARY="riscv64-linux-gnu-ld"

EXPOSE 3000

ENTRYPOINT ["./target/release/risc-v-sim-web"]