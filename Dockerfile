FROM --platform=linux/amd64 rust:1.85.1-bookworm@sha256:e51d0265072d2d9d5d320f6a44dde6b9ef13653b035098febd68cce8fa7c0bc4

RUN apt-get update && apt-get install -y --no-install-recommends python3 bzip2 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /opt/platform-tools /cargo /build \
    && curl --fail --location --retry 3 --output /tmp/platform-tools.tar.bz2 \
       https://github.com/anza-xyz/platform-tools/releases/download/v1.57/platform-tools-linux-x86_64.tar.bz2 \
    && echo 'b0f7af104adf726fff2a6a09ea2eb2f2d2965c92295f4d7388c08d140e0c2b00  /tmp/platform-tools.tar.bz2' | sha256sum -c - \
    && tar -xjf /tmp/platform-tools.tar.bz2 -C /opt/platform-tools \
    && rm /tmp/platform-tools.tar.bz2
ENV STONK_PLATFORM_TOOLS=/opt/platform-tools
ENV CARGO_HOME=/cargo
ENV PATH="/opt/platform-tools/rust/bin:/opt/platform-tools/llvm/bin:/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin"
COPY build-support/cargo-build-sbf /usr/local/bin/cargo-build-sbf
RUN chmod 755 /usr/local/bin/cargo-build-sbf
WORKDIR /build
CMD ["bash"]
