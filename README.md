# Rust Script - Terminal Session Recorder

This is a Rust implementation of the classic Unix `script` command, which records terminal sessions to a file.

## Features

- Records terminal input/output to typescript files
- Supports timing information logging (simple and advanced formats)
- Multiple logging formats (raw data, timing information)
- Configurable output limits
- Signal handling and window size changes
- Cross-platform support (Unix-like systems)

## Usage

```bash
# Basic usage - record session to 'typescript' file
cargo run

# Record to specific file
cargo run -- output.txt

# Record with timing information
cargo run -- -T timing.txt output.txt

# Record both input and output to same file
cargo run -- -B session.log

# Run specific command instead of interactive shell
cargo run -- -c "ls -la" output.txt

# Quiet mode
cargo run -- -q output.txt

# Set output size limit
cargo run -- -o 1MB output.txt
```

## Command Line Options

- `-I, --log-in <file>`: Log stdin to file
- `-O, --log-out <file>`: Log stdout to file (default)
- `-B, --log-io <file>`: Log stdin and stdout to file
- `-T, --log-timing <file>`: Log timing information to file
- `-t, --timing[=<file>]`: Deprecated alias to -T (default file is stderr)
- `-m, --logging-format <format>`: Force to 'classic' or 'advanced' format
- `-a, --append`: Append to the log file
- `-c, --command <command>`: Run command rather than interactive shell
- `-e, --return`: Return exit code of the child process
- `-f, --flush`: Run flush after each write
- `--force`: Use output file even when it is a link
- `-E, --echo <when>`: Echo input in session (auto, always or never)
- `-o, --output-limit <size>`: Terminate if output files exceed size
- `-q, --quiet`: Be quiet

## Architecture

The Rust implementation is organized into several modules:

### `main.rs`
Entry point that handles command-line argument parsing using `clap`.

### `script_control.rs`
Main control structure that manages the overall script session, including:
- Configuration management
- Logging setup
- Process forking and management
- Signal handling
- Main event loop

### `pty_session.rs`
Handles pseudo-terminal (PTY) operations:
- PTY creation and setup
- Terminal mode management
- Window size handling
- File descriptor management

### `logging.rs`
Manages different logging formats and file operations:
- Raw data logging
- Simple timing format
- Advanced multi-stream timing format
- Signal and info logging

### `utils.rs`
Utility functions for:
- Terminal detection and information
- Size parsing
- File link checking
- Platform-specific operations

## Dependencies

- `clap`: Command-line argument parsing
- `nix`: Unix system calls and PTY operations
- `tokio`: Async runtime for signal handling
- `chrono`: Date and time handling
- `anyhow`/`thiserror`: Error handling
- `signal-hook`: Signal handling utilities
- `termios`: Terminal I/O settings

## Building

```bash
cd rust_script
cargo build --release
```

## Testing

```bash
cargo test
```

## Differences from C Implementation

1. **Async/Await**: Uses Tokio for async signal handling and I/O operations
2. **Memory Safety**: Rust's ownership system prevents common C memory issues
3. **Error Handling**: Uses Result types for robust error handling
4. **Modular Design**: Clean separation of concerns across modules
5. **Type Safety**: St

## License
This implementation uses Apache 2.0, check out the LICENSE file.