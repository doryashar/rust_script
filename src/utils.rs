use anyhow::{anyhow, Result};
use nix::pty::Winsize;
use std::fs;
use std::path::Path;

pub fn is_stdin_tty() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

pub fn get_terminal_size() -> Result<(u16, u16)> {
    let winsize = get_winsize()?;
    Ok((winsize.ws_col, winsize.ws_row))
}

pub fn get_winsize() -> Result<Winsize> {
    let mut winsize = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    unsafe {
        let ret = libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize);
        if ret == -1 {
            // Default size if ioctl fails
            winsize.ws_row = 24;
            winsize.ws_col = 80;
        }
    }

    Ok(winsize)
}

pub fn get_terminal_name() -> Option<String> {
    unsafe {
        let tty_name = libc::ttyname(libc::STDIN_FILENO);
        if tty_name.is_null() {
            None
        } else {
            let c_str = std::ffi::CStr::from_ptr(tty_name);
            c_str.to_str().ok().map(|s| s.to_string())
        }
    }
}

pub fn get_terminal_type() -> Option<String> {
    std::env::var("TERM").ok()
}

pub fn parse_size(size_str: &str) -> Result<u64> {
    let size_str = size_str.trim().to_lowercase();
    
    if size_str.is_empty() {
        return Err(anyhow!("Empty size string"));
    }

    let (number_part, suffix) = if size_str.ends_with("k") || size_str.ends_with("kb") {
        let num_str = if size_str.ends_with("kb") {
            &size_str[..size_str.len()-2]
        } else {
            &size_str[..size_str.len()-1]
        };
        (num_str, 1024u64)
    } else if size_str.ends_with("m") || size_str.ends_with("mb") {
        let num_str = if size_str.ends_with("mb") {
            &size_str[..size_str.len()-2]
        } else {
            &size_str[..size_str.len()-1]
        };
        (num_str, 1024u64 * 1024)
    } else if size_str.ends_with("g") || size_str.ends_with("gb") {
        let num_str = if size_str.ends_with("gb") {
            &size_str[..size_str.len()-2]
        } else {
            &size_str[..size_str.len()-1]
        };
        (num_str, 1024u64 * 1024 * 1024)
    } else {
        (&size_str[..], 1u64)
    };

    let number: u64 = number_part.parse()
        .map_err(|_| anyhow!("Invalid number in size: {}", number_part))?;

    Ok(number * suffix)
}

pub fn die_if_link<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(anyhow!(
                "output file `{}' is a link\nUse --force if you really want to use it.\nProgram not started.",
                path.display()
            ));
        }
        
        // Check for hard links (nlink > 1)
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() > 1 {
                return Err(anyhow!(
                    "output file `{}' is a link\nUse --force if you really want to use it.\nProgram not started.",
                    path.display()
                ));
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("1k").unwrap(), 1024);
        assert_eq!(parse_size("1kb").unwrap(), 1024);
        assert_eq!(parse_size("1m").unwrap(), 1024 * 1024);
        assert_eq!(parse_size("1mb").unwrap(), 1024 * 1024);
        assert_eq!(parse_size("1g").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("1gb").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("2K").unwrap(), 2 * 1024);
        assert_eq!(parse_size("5M").unwrap(), 5 * 1024 * 1024);
    }
}