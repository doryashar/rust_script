#!/bin/bash

echo "Starting rust_script test..."
echo "In the session, type 'hello' then press Ctrl+C to see if it starts a new line"
echo "Then type 'exit' to quit the session"
echo ""

# Start the script
./target/debug/rust_script /tmp/test_ctrl_c.txt