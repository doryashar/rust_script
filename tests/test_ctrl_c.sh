#!/bin/bash

echo "Testing Ctrl+C behavior..."
echo "Type some text and press Ctrl+C to see if it starts a new line"
echo "Then type 'exit' to quit"

# Run the script command
./target/debug/rust_script /tmp/test_session.txt