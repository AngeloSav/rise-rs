# Justfile for PEF project

build:
    RUSTFLAGS="-C target-cpu=native" cargo build --release