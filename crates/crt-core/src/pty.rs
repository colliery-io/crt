//! PTY (Pseudo-Terminal) management
//!
//! Handles spawning shell processes and I/O between the shell and terminal.

use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};

/// Messages sent to the PTY writer thread
pub enum PtyInput {
    /// Data to write to the PTY
    Data(Vec<u8>),
    /// Resize the PTY
    Resize { cols: u16, rows: u16 },
    /// Shutdown the PTY
    Shutdown,
}

/// PTY handle for communicating with a shell process
pub struct Pty {
    /// Channel to send input to the PTY
    input_tx: Sender<PtyInput>,
    /// Channel to receive output from the PTY
    output_rx: Receiver<Vec<u8>>,
    /// Child process handle
    _child: Box<dyn Child + Send + Sync>,
}

impl Pty {
    /// Spawn a new shell in a PTY
    pub fn spawn(shell: Option<&str>, cols: u16, rows: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(size)?;

        // Determine shell to use
        let shell = shell
            .map(String::from)
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| String::from("/bin/sh"));

        let cmd = CommandBuilder::new(&shell);
        let child = pair.slave.spawn_command(cmd)?;

        // Set up channels for communication
        let (input_tx, input_rx) = mpsc::channel::<PtyInput>();
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();

        // Get reader and writer from master
        let mut reader = pair.master.try_clone_reader()?;
        let master = pair.master;

        // Spawn reader thread - reads PTY output and sends to channel
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if output_tx.send(buf[..n].to_vec()).is_err() {
                            break; // Channel closed
                        }
                    }
                    Err(e) => {
                        log::error!("PTY read error: {}", e);
                        break;
                    }
                }
            }
            log::debug!("PTY reader thread exiting");
        });

        // Spawn writer thread - receives from channel and writes to PTY
        thread::spawn(move || {
            let mut writer = master.take_writer().expect("Failed to get PTY writer");

            for msg in input_rx {
                match msg {
                    PtyInput::Data(data) => {
                        if let Err(e) = writer.write_all(&data) {
                            log::error!("PTY write error: {}", e);
                            break;
                        }
                        let _ = writer.flush();
                    }
                    PtyInput::Resize { cols, rows } => {
                        let size = PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        };
                        if let Err(e) = master.resize(size) {
                            log::error!("PTY resize error: {}", e);
                        }
                    }
                    PtyInput::Shutdown => {
                        log::debug!("PTY writer thread shutting down");
                        break;
                    }
                }
            }
            log::debug!("PTY writer thread exiting");
        });

        Ok(Self {
            input_tx,
            output_rx,
            _child: child,
        })
    }

    /// Write data to the PTY (keyboard input)
    pub fn write(&self, data: &[u8]) {
        let _ = self.input_tx.send(PtyInput::Data(data.to_vec()));
    }

    /// Try to read available output from the PTY (non-blocking)
    pub fn try_read(&self) -> Option<Vec<u8>> {
        self.output_rx.try_recv().ok()
    }

    /// Read all available output from the PTY (non-blocking)
    pub fn read_available(&self) -> Vec<u8> {
        let mut output = Vec::new();
        while let Ok(data) = self.output_rx.try_recv() {
            output.extend(data);
        }
        output
    }

    /// Resize the PTY
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.input_tx.send(PtyInput::Resize { cols, rows });
    }

    /// Shutdown the PTY
    pub fn shutdown(&self) {
        let _ = self.input_tx.send(PtyInput::Shutdown);
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn spawn_shell() {
        let pty = Pty::spawn(Some("/bin/sh"), 80, 24).expect("Failed to spawn PTY");

        // Give the shell time to start
        thread::sleep(Duration::from_millis(100));

        // Send a simple command
        pty.write(b"echo hello\n");

        // Wait for output
        thread::sleep(Duration::from_millis(100));

        let output = pty.read_available();
        assert!(!output.is_empty(), "Should have received some output");

        // Output should contain "hello"
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("hello"), "Output should contain 'hello': {}", output_str);
    }
}
