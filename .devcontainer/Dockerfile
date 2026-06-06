FROM mcr.microsoft.com/devcontainers/typescript-node:4-24-bookworm

USER root

RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    pkg-config \
    clang \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    nasm \
    yasm \
    cmake \
    git \
    fonts-ipafont-gothic \
    fonts-ipafont-mincho \
    dbus-x11 \
    dconf-cli \
    && rm -rf /var/lib/apt/lists/*

RUN git config --system core.autocrlf false


