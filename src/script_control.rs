use anyhow::{anyhow, Result};
use chrono::Local;
use nix::unistd::{fork, ForkResult};
use std::os::unix::io::RawFd;
use std::path::PathBuf;
use tokio::signal;

use crate::logging::{LogFormat, ScriptLogger};
use crate::pty_session::PtySession;
use crate::utils;
use crate::Args;

const DEFAULT_TYPESCRIPT_FILENAME: &str = "typescript";

pub struct ScriptControl {
    // Output and input streams
    pub out_logs: Vec<ScriptLogger>,
    pub in_logs: Vec<ScriptLogger>,
    
    // Signal and info logs
    pub sig_log: Option<ScriptLogger>,
    pub info_log: Option<ScriptLogger>,
    
    // Terminal information
    pub tty_name: Option<String>,
    pub tty_type: Option<String>,
    pub command: Option<String>,
    pub command_norm: Option<String>,
    pub tty_cols: u16,
    pub tty_lines: u16,
    
    // PTY session
    pub pty: Option<PtySession>,
    pub child_pid: Option<nix::unistd::Pid>,
    pub child_status: Option<i32>,
    
    // Configuration flags
    pub append: bool,
    pub rc_wanted: bool,
    pub flush: bool,
    pub quiet: bool,
    pub force: bool,
    pub is_term: bool,
    
    // Output size tracking
    pub out_size: u64,
    pub max_size: u64,
}

impl ScriptControl {
    pub fn new(args: Args) -> Result<Self> {
        let is_term = utils::is_stdin_tty();
        let (tty_cols, tty_lines) = if is_term {
            utils::get_terminal_size()?
        } else {
            (80, 24)
        };

        let mut control = ScriptControl {
            out_logs: Vec::new(),
            in_logs: Vec::new(),
            sig_log: None,
            info_log: None,
            tty_name: None,
            tty_type: None,
            command: args.command.clone(),
            command_norm: args.command.as_ref().map(|c| c.replace('\n', " ")),
            tty_cols,
            tty_lines,
            pty: None,
            child_pid: None,
            child_status: None,
            append: args.append,
            rc_wanted: args.return_exit_code,
            flush: args.flush,
            quiet: args.quiet,
            force: args.force,
            is_term,
            out_size: 0,
            max_size: if let Some(ref limit) = args.output_limit {
                utils::parse_size(&limit)?
            } else {
                0
            },
        };

        // Initialize terminal info if we're on a terminal
        if is_term {
            control.init_terminal_info()?;
        }

        // Set up logging based on arguments
        control.setup_logging(args)?;

        Ok(control)
    }

    fn init_terminal_info(&mut self) -> Result<()> {
        self.tty_name = utils::get_terminal_name();
        self.tty_type = utils::get_terminal_type();
        Ok(())
    }

    fn setup_logging(&mut self, args: Args) -> Result<()> {
        let mut outfile = None;
        let mut infile = None;
        let mut timingfile = None;
        let mut format = LogFormat::Raw;

        // Handle log-io option (both input and output)
        if let Some(path) = args.log_io {
            self.associate_log(&path, LogFormat::Raw, true, true)?;
            outfile = Some(path.clone());
            infile = Some(path);
        }

        // Handle log-in option
        if let Some(path) = args.log_in {
            self.associate_log(&path, LogFormat::Raw, true, false)?;
            infile = Some(path);
        }

        // Handle log-out option
        if let Some(path) = args.log_out {
            self.associate_log(&path, LogFormat::Raw, false, true)?;
            outfile = Some(path);
        }

        // Handle timing options
        if let Some(path) = args.log_timing {
            timingfile = Some(path);
        } else if let Some(timing_opt) = args.timing {
            timingfile = timing_opt.or_else(|| Some(PathBuf::from("/dev/stderr")));
        }

        // Determine timing format
        if let Some(fmt_str) = args.logging_format {
            format = match fmt_str.to_lowercase().as_str() {
                "classic" => LogFormat::TimingSimple,
                "advanced" => LogFormat::TimingMulti,
                _ => return Err(anyhow!("Unsupported logging format: '{}'", fmt_str)),
            };
        } else if timingfile.is_some() {
            // Auto-detect format based on whether we have both input and output
            format = if infile.is_some() && outfile.is_some() {
                LogFormat::TimingMulti
            } else {
                LogFormat::TimingSimple
            };
        }

        // Set up timing logs
        if let Some(path) = timingfile {
            if outfile.is_some() {
                self.associate_log(&path, format, false, true)?;
            }
            if infile.is_some() {
                self.associate_log(&path, format, true, false)?;
            }
        }

        // Default output file if none specified
        if outfile.is_none() && infile.is_none() {
            let default_file = args.file.unwrap_or_else(|| PathBuf::from(DEFAULT_TYPESCRIPT_FILENAME));
            
            if !self.force {
                utils::die_if_link(&default_file)?;
            }
            
            self.associate_log(&default_file, LogFormat::Raw, false, true)?;
        }

        Ok(())
    }

    fn associate_log(&mut self, path: &PathBuf, format: LogFormat, is_input: bool, is_output: bool) -> Result<()> {
        let logger = ScriptLogger::new(path.clone(), format, self.append)?;

        if is_input {
            self.in_logs.push(logger.clone());
        }
        if is_output {
            self.out_logs.push(logger.clone());
        }

        // Set up signal and info logs for multi-stream timing
        if format == LogFormat::TimingMulti {
            if self.sig_log.is_none() {
                self.sig_log = Some(logger.clone());
            }
            if self.info_log.is_none() {
                self.info_log = Some(logger);
            }
        }

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        // Create PTY session
        self.pty = Some(PtySession::new(self.is_term)?);

        if !self.quiet {
            println!("Script started");
            // TODO: Print log file information
            println!(".");
        }

        // Set up the PTY
        if let Some(ref mut pty) = self.pty {
            pty.setup()?;
        }

        // Fork the child process
        match unsafe { fork() }? {
            ForkResult::Parent { child } => {
                self.child_pid = Some(child);
                self.run_parent().await?;
            }
            ForkResult::Child => {
                self.run_child()?;
            }
        }

        Ok(())
    }

    async fn run_parent(&mut self) -> Result<()> {
        // Start logging
        self.start_logging().await?;

        // Start I/O proxy
        if let Some(ref pty) = self.pty {
            self.proxy_io(pty.get_master_fd()).await?;
        }

        // Stop logging
        self.stop_logging().await?;

        if !self.quiet {
            println!("Script done.");
        }

        Ok(())
    }

    async fn proxy_io(&mut self, master_fd: RawFd) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        // Set master fd to non-blocking
        let flags = nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_GETFL)?;
        nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::from_bits_truncate(flags) | nix::fcntl::OFlag::O_NONBLOCK))?;
        
        // Set up signal handling
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        let mut sigwinch = signal::unix::signal(signal::unix::SignalKind::window_change())?;
        
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        
        let mut stdin_buf = [0u8; 8192];
        let mut master_buf = [0u8; 8192];
        
        loop {
            tokio::select! {
                // Handle signals
                _ = sigterm.recv() => {
                    self.handle_signal("SIGTERM").await?;
                    // Forward SIGTERM to child process
                    if let Some(child_pid) = self.child_pid {
                        let _ = nix::sys::signal::kill(child_pid, nix::sys::signal::Signal::SIGTERM);
                    }
                    break;
                }
                _ = sigwinch.recv() => {
                    self.handle_window_change().await?;
                }
                
                // Read from stdin and write to master
                result = stdin.read(&mut stdin_buf) => {
                    match result {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            // Log input
                            self.log_input(&stdin_buf[..n]).await?;
                            
                            // Write to master PTY
                            let bytes_written = nix::unistd::write(master_fd, &stdin_buf[..n])?;
                            if bytes_written != n {
                                return Err(anyhow!("Partial write to master PTY"));
                            }
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                
                // Read from master and write to stdout
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    // Use non-blocking read from master
                    match nix::unistd::read(master_fd, &mut master_buf) {
                        Ok(n) if n > 0 => {
                            // Log output
                            self.log_output(&master_buf[..n]).await?;
                            
                            // Write to stdout
                            stdout.write_all(&master_buf[..n]).await?;
                            stdout.flush().await?;
                        }
                        Ok(0) => break, // EOF
                        Ok(_) => {}, // Zero bytes read but not EOF
                        Err(e) if e == nix::errno::Errno::EAGAIN || e == nix::errno::Errno::EWOULDBLOCK => {
                            // No data available, continue
                        }
                        Err(e) => return Err(anyhow!("Error reading from master PTY: {}", e)),
                    }
                }
            }
            
            // Check if child has exited
            if let Some(child_pid) = self.child_pid {
                match nix::sys::wait::waitpid(child_pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG))? {
                    nix::sys::wait::WaitStatus::StillAlive => {
                        // Child still running, continue
                    }
                    status => {
                        // Child has exited
                        match status {
                            nix::sys::wait::WaitStatus::Exited(_, code) => {
                                self.child_status = Some(code);
                            }
                            nix::sys::wait::WaitStatus::Signaled(_, signal, _) => {
                                self.child_status = Some(128 + signal as i32);
                            }
                            _ => {
                                self.child_status = Some(1);
                            }
                        }
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn log_input(&mut self, data: &[u8]) -> Result<()> {
        for logger in &mut self.in_logs {
            let size = logger.log_data(crate::logging::LogStream::Input, data).await?;
            self.out_size += size as u64;
            
            // Check output limit
            if self.max_size > 0 && self.out_size >= self.max_size {
                if !self.quiet {
                    println!("Script terminated, max output files size {} exceeded.", self.max_size);
                }
                return Err(anyhow!("Output size limit exceeded"));
            }
        }
        Ok(())
    }

    async fn log_output(&mut self, data: &[u8]) -> Result<()> {
        for logger in &mut self.out_logs {
            let size = logger.log_data(crate::logging::LogStream::Output, data).await?;
            self.out_size += size as u64;
            
            // Check output limit
            if self.max_size > 0 && self.out_size >= self.max_size {
                if !self.quiet {
                    println!("Script terminated, max output files size {} exceeded.", self.max_size);
                }
                return Err(anyhow!("Output size limit exceeded"));
            }
        }
        Ok(())
    }

    fn run_child(&self) -> Result<()> {
        // Initialize slave PTY
        if let Some(ref pty) = self.pty {
            pty.init_slave()?;
        }

        // Execute shell or command
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let shell_clone = shell.clone();
        let shell_name = std::path::Path::new(&shell_clone)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sh");

        if let Some(ref command) = self.command {
            // Execute specific command
            let args = [shell_name, "-c", command.as_str()];
            let c_shell = std::ffi::CString::new(shell.clone())?;
            let c_args: Vec<std::ffi::CString> = args.iter()
                .map(|&s| std::ffi::CString::new(s).unwrap())
                .collect();
            nix::unistd::execv(&c_shell, &c_args)?;
        } else {
            // Execute interactive shell
            let args = [shell_name, "-i"];
            let c_shell = std::ffi::CString::new(shell.clone())?;
            let c_args: Vec<std::ffi::CString> = args.iter()
                .map(|&s| std::ffi::CString::new(s).unwrap())
                .collect();
            nix::unistd::execv(&c_shell, &c_args)?;
        }

        // Should never reach here
        Err(anyhow!("Failed to execute shell"))
    }

    async fn start_logging(&mut self) -> Result<()> {
        // Collect the data we need before borrowing mutably
        let is_term = self.is_term;
        let tty_type = self.tty_type.clone();
        let tty_name = self.tty_name.clone();
        let tty_cols = self.tty_cols;
        let tty_lines = self.tty_lines;
        let command_norm = self.command_norm.clone();
        
        // Start all output loggers
        for i in 0..self.out_logs.len() {
            self.out_logs[i].start_with_data(
                is_term, 
                &tty_type, 
                &tty_name, 
                tty_cols, 
                tty_lines, 
                &command_norm
            ).await?;
        }
        
        // Start all input loggers
        for i in 0..self.in_logs.len() {
            self.in_logs[i].start_with_data(
                is_term, 
                &tty_type, 
                &tty_name, 
                tty_cols, 
                tty_lines, 
                &command_norm
            ).await?;
        }

        // Log initial info for multi-stream timing
        if let Some(ref mut info_log) = self.info_log {
            let now = Local::now();
            info_log.log_info("START_TIME", &now.to_rfc3339()).await?;
            
            if is_term {
                if let Some(ref tty_type) = tty_type {
                    info_log.log_info("TERM", tty_type).await?;
                }
                if let Some(ref tty_name) = tty_name {
                    info_log.log_info("TTY", tty_name).await?;
                }
                info_log.log_info("COLUMNS", &tty_cols.to_string()).await?;
                info_log.log_info("LINES", &tty_lines.to_string()).await?;
            }
            
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            info_log.log_info("SHELL", &shell).await?;
            
            if let Some(ref command) = command_norm {
                info_log.log_info("COMMAND", command).await?;
            }
        }

        Ok(())
    }

    async fn stop_logging(&mut self) -> Result<()> {
        let status = self.child_status.unwrap_or(0);
        
        // Close all loggers
        for logger in &mut self.out_logs {
            logger.close(status).await?;
        }
        for logger in &mut self.in_logs {
            logger.close(status).await?;
        }

        Ok(())
    }

    async fn handle_signal(&mut self, signal_name: &str) -> Result<()> {
        if let Some(ref mut sig_log) = self.sig_log {
            sig_log.log_signal(signal_name, None).await?;
        }
        Ok(())
    }

    async fn handle_window_change(&mut self) -> Result<()> {
        let (cols, lines) = utils::get_terminal_size()?;
        self.tty_cols = cols;
        self.tty_lines = lines;

        if let Some(ref mut sig_log) = self.sig_log {
            let msg = format!("ROWS={} COLS={}", lines, cols);
            sig_log.log_signal("SIGWINCH", Some(&msg)).await?;
        }

        // Update PTY window size
        if let Some(ref mut pty) = self.pty {
            pty.set_window_size(cols, lines)?;
        }

        Ok(())
    }

    async fn wait_for_child(&mut self) -> Result<()> {
        if let Some(child_pid) = self.child_pid {
            match nix::sys::wait::waitpid(child_pid, None)? {
                nix::sys::wait::WaitStatus::Exited(_, status) => {
                    self.child_status = Some(status);
                }
                nix::sys::wait::WaitStatus::Signaled(_, signal, _) => {
                    self.child_status = Some(128 + signal as i32);
                }
                _ => {
                    self.child_status = Some(1);
                }
            }
        }
        Ok(())
    }
}