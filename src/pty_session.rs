use anyhow::{anyhow, Result};
use nix::pty::{openpty, Winsize};
use nix::unistd::{close, dup2};
use std::os::unix::io::{IntoRawFd, RawFd};
use termios::{Termios, tcsetattr, TCSANOW};

pub struct PtySession {
    pub master_fd: RawFd,
    pub slave_fd: RawFd,
    pub is_term: bool,
    pub original_termios: Option<Termios>,
    pub window_size: Winsize,
}

impl PtySession {
    pub fn new(is_term: bool) -> Result<Self> {
        // Get current window size if we're on a terminal
        let window_size = if is_term {
            crate::utils::get_winsize()?
        } else {
            Winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }
        };

        // Save original terminal settings
        let original_termios = if is_term {
            let termios = Termios::from_fd(libc::STDIN_FILENO)?;
            Some(termios)
        } else {
            None
        };

        // Create PTY pair
        let pty_result = openpty(&window_size, None)?;

        // Convert OwnedFd to RawFd
        let master_fd = pty_result.master.into_raw_fd();
        let slave_fd = pty_result.slave.into_raw_fd();

        Ok(PtySession {
            master_fd,
            slave_fd,
            is_term,
            original_termios,
            window_size,
        })
    }

    pub fn setup(&mut self) -> Result<()> {
        if self.is_term {
            // Set terminal to raw mode
            let mut termios = Termios::from_fd(libc::STDIN_FILENO)?;
            
            // Make terminal raw
            termios::cfmakeraw(&mut termios);
            
            // Apply the settings
            tcsetattr(libc::STDIN_FILENO, TCSANOW, &termios)?;
        }

        Ok(())
    }

    pub fn init_slave(&self) -> Result<()> {
        // Close master fd in child
        close(self.master_fd)?;

        // Create new session first
        nix::unistd::setsid()?;
        
        // Make this PTY the controlling terminal
        unsafe {
            let ret = libc::ioctl(self.slave_fd, libc::TIOCSCTTY, 0);
            if ret == -1 {
                return Err(anyhow!("Failed to set controlling terminal"));
            }
        }

        // Redirect stdin, stdout, stderr to slave
        dup2(self.slave_fd, libc::STDIN_FILENO)?;
        dup2(self.slave_fd, libc::STDOUT_FILENO)?;
        dup2(self.slave_fd, libc::STDERR_FILENO)?;

        // Close the original slave fd if it's not one of the standard fds
        if self.slave_fd > 2 {
            close(self.slave_fd)?;
        }

        // Set the slave terminal to have normal (cooked) mode settings
        // so that Ctrl+C works properly in the child process
        let mut termios = Termios::from_fd(libc::STDIN_FILENO)?;
        
        // Reset to sane defaults for the child
        termios.c_iflag = libc::ICRNL | libc::IXON;
        termios.c_oflag = libc::OPOST | libc::ONLCR;
        termios.c_cflag = libc::CS8 | libc::CREAD | libc::CLOCAL;
        termios.c_lflag = libc::ISIG | libc::ICANON | libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ECHOCTL | libc::ECHOKE;
        
        // Set control characters
        termios.c_cc[libc::VINTR] = 3;    // Ctrl+C
        termios.c_cc[libc::VQUIT] = 28;   // Ctrl+\
        termios.c_cc[libc::VERASE] = 127; // DEL
        termios.c_cc[libc::VKILL] = 21;   // Ctrl+U
        termios.c_cc[libc::VEOF] = 4;     // Ctrl+D
        termios.c_cc[libc::VSTART] = 17;  // Ctrl+Q
        termios.c_cc[libc::VSTOP] = 19;   // Ctrl+S
        termios.c_cc[libc::VSUSP] = 26;   // Ctrl+Z
        
        tcsetattr(libc::STDIN_FILENO, TCSANOW, &termios)?;

        Ok(())
    }

    pub fn set_window_size(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.window_size.ws_col = cols;
        self.window_size.ws_row = rows;

        // Update the PTY window size
        unsafe {
            let ret = libc::ioctl(
                self.master_fd,
                libc::TIOCSWINSZ,
                &self.window_size as *const Winsize,
            );
            if ret == -1 {
                return Err(anyhow!("Failed to set window size"));
            }
        }

        Ok(())
    }

    pub fn get_master_fd(&self) -> RawFd {
        self.master_fd
    }

    pub fn get_slave_fd(&self) -> RawFd {
        self.slave_fd
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        // Restore original terminal settings
        if let Some(ref termios) = self.original_termios {
            let _ = tcsetattr(libc::STDIN_FILENO, TCSANOW, termios);
        }

        // Close file descriptors
        let _ = close(self.master_fd);
        let _ = close(self.slave_fd);
    }
}