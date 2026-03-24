FROM rust:1.87-bookworm

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# wasm-pack
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# wasm target
RUN rustup target add wasm32-unknown-unknown

# Node.js (for web frontend)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# Rust tools
RUN rustup component add clippy rustfmt

# Default dev user (UID/GID adjusted at runtime by entrypoint.sh)
RUN groupadd -g 1000 dev \
    && useradd -m -u 1000 -g dev -s /bin/bash dev \
    && mkdir -p /workspace && chown dev:dev /workspace

# Make Rust toolchain accessible to dev user
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
RUN chmod -R a+rw ${RUSTUP_HOME} ${CARGO_HOME}

# Install gosu for entrypoint user switching
RUN apt-get update && apt-get install -y gosu && rm -rf /var/lib/apt/lists/*

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["bash"]
