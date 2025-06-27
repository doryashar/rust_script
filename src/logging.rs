use anyhow::{anyhow, Result};
use chrono::Local;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogFormat {
    Raw,
    TimingSimple,
    TimingMulti,
}

#[derive(Debug, Clone)]
pub enum LogStream {
    Input,
    Output,
}

#[derive(Clone)]
pub struct ScriptLogger {
    path: PathBuf,
    format: LogFormat,
    append: bool,
    writer: Arc<Mutex<Option<BufWriter<std::fs::File>>>>,
    start_time: Arc<Mutex<Option<Instant>>>,
    last_time: Arc<Mutex<Option<Instant>>>,
    initialized: Arc<Mutex<bool>>,
}

impl ScriptLogger {
    pub fn new(path: PathBuf, format: LogFormat, append: bool) -> Result<Self> {
        Ok(ScriptLogger {
            path,
            format,
            append,
            writer: Arc::new(Mutex::new(None)),
            start_time: Arc::new(Mutex::new(None)),
            last_time: Arc::new(Mutex::new(None)),
            initialized: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn start_with_data(
        &mut self, 
        is_term: bool,
        tty_type: &Option<String>,
        tty_name: &Option<String>,
        tty_cols: u16,
        tty_lines: u16,
        command_norm: &Option<String>
    ) -> Result<()> {
        let mut initialized = self.initialized.lock().unwrap();
        if *initialized {
            return Ok(());
        }

        // Open the file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(self.append && self.format == LogFormat::Raw)
            .truncate(!self.append || self.format != LogFormat::Raw)
            .open(&self.path)?;

        let mut writer = BufWriter::new(file);

        // Write header based on format
        match self.format {
            LogFormat::Raw => {
                let now = Local::now();
                writeln!(writer, "Script started on {} [", now.format("%Y-%m-%d %H:%M:%S %z"))?;

                if let Some(ref command) = command_norm {
                    write!(writer, "COMMAND=\"{}\"", command)?;
                }

                if is_term {
                    if let Some(ref tty_type) = tty_type {
                        write!(writer, " TERM=\"{}\"", tty_type)?;
                    }
                    if let Some(ref tty_name) = tty_name {
                        write!(writer, " TTY=\"{}\"", tty_name)?;
                    }
                    write!(writer, " COLUMNS=\"{}\" LINES=\"{}\"", tty_cols, tty_lines)?;
                } else {
                    write!(writer, " <not executed on terminal>")?;
                }

                writeln!(writer, "]")?;
            }
            LogFormat::TimingSimple | LogFormat::TimingMulti => {
                // Initialize timing
                let now = Instant::now();
                *self.start_time.lock().unwrap() = Some(now);
                *self.last_time.lock().unwrap() = Some(now);
            }
        }

        *self.writer.lock().unwrap() = Some(writer);
        *initialized = true;

        Ok(())
    }

    pub async fn log_data(&mut self, stream: LogStream, data: &[u8]) -> Result<usize> {
        let mut writer_guard = self.writer.lock().unwrap();
        let writer = writer_guard.as_mut().ok_or_else(|| anyhow!("Logger not initialized"))?;

        match self.format {
            LogFormat::Raw => {
                writer.write_all(data)?;
                writer.flush()?;
                Ok(data.len())
            }
            LogFormat::TimingSimple => {
                let now = Instant::now();
                let mut last_time = self.last_time.lock().unwrap();
                let delta = if let Some(last) = *last_time {
                    now.duration_since(last)
                } else {
                    Duration::from_secs(0)
                };

                writeln!(writer, "{:.6} {}", 
                    delta.as_secs_f64(), 
                    data.len())?;
                writer.flush()?;

                *last_time = Some(now);
                Ok(format!("{:.6} {}\n", delta.as_secs_f64(), data.len()).len())
            }
            LogFormat::TimingMulti => {
                let now = Instant::now();
                let mut last_time = self.last_time.lock().unwrap();
                let delta = if let Some(last) = *last_time {
                    now.duration_since(last)
                } else {
                    Duration::from_secs(0)
                };

                let stream_char = match stream {
                    LogStream::Input => 'I',
                    LogStream::Output => 'O',
                };

                writeln!(writer, "{} {:.6} {}", 
                    stream_char,
                    delta.as_secs_f64(), 
                    data.len())?;
                writer.flush()?;

                *last_time = Some(now);
                Ok(format!("{} {:.6} {}\n", stream_char, delta.as_secs_f64(), data.len()).len())
            }
        }
    }

    pub async fn log_signal(&mut self, signal_name: &str, message: Option<&str>) -> Result<()> {
        if self.format != LogFormat::TimingMulti {
            return Ok(());
        }

        let mut writer_guard = self.writer.lock().unwrap();
        let writer = writer_guard.as_mut().ok_or_else(|| anyhow!("Logger not initialized"))?;

        let now = Instant::now();
        let mut last_time = self.last_time.lock().unwrap();
        let delta = if let Some(last) = *last_time {
            now.duration_since(last)
        } else {
            Duration::from_secs(0)
        };

        if let Some(msg) = message {
            writeln!(writer, "S {:.6} {} {}", delta.as_secs_f64(), signal_name, msg)?;
        } else {
            writeln!(writer, "S {:.6} {}", delta.as_secs_f64(), signal_name)?;
        }
        writer.flush()?;

        *last_time = Some(now);
        Ok(())
    }

    pub async fn log_info(&mut self, name: &str, value: &str) -> Result<()> {
        if self.format != LogFormat::TimingMulti {
            return Ok(());
        }

        let mut writer_guard = self.writer.lock().unwrap();
        let writer = writer_guard.as_mut().ok_or_else(|| anyhow!("Logger not initialized"))?;

        writeln!(writer, "H 0.0 {} {}", name, value)?;
        writer.flush()?;

        Ok(())
    }

    pub async fn close(&mut self, exit_status: i32) -> Result<()> {
        let mut writer_guard = self.writer.lock().unwrap();
        if let Some(mut writer) = writer_guard.take() {
            match self.format {
                LogFormat::Raw => {
                    let now = Local::now();
                    writeln!(writer, "\nScript done on {} [COMMAND_EXIT_CODE=\"{}\"]", 
                        now.format("%Y-%m-%d %H:%M:%S %z"), 
                        exit_status)?;
                }
                LogFormat::TimingMulti => {
                    let now = Instant::now();
                    let start_time = self.start_time.lock().unwrap();
                    if let Some(start) = *start_time {
                        let duration = now.duration_since(start);
                        writeln!(writer, "H 0.0 DURATION {:.6}", duration.as_secs_f64())?;
                        writeln!(writer, "H 0.0 EXIT_CODE {}", exit_status)?;
                    }
                }
                LogFormat::TimingSimple => {
                    // No special closing for simple timing format
                }
            }
            writer.flush()?;
        }
        Ok(())
    }
}