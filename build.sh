#!/bin/bash

# Build script for rust_script

echo "Building rust_script..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo is not installed. Please install Rust first."
    echo "Visit https://rustup.rs/ for installation instructions."
    exit 1
fi

# Build in release mode
cargo build --release

if [ $? -eq 0 ]; then
    echo "Build successful!"
    echo "Binary location: target/release/rust_script"
    echo ""
    echo "To install system-wide, run:"
    echo "  sudo cp target/release/rust_script /usr/local/bin/script-rs"
    echo ""
    echo "To test, run:"
    echo "  ./target/release/rust_script"
else
    echo "Build failed!"
    exit 1
fi