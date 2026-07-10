//! Meshtastic transport via line-oriented stdio bridge.
//!
//! Protocol with `tools/meshtastic_bridge.py`:
//!   → send:  `TXHEX <hex>\n`
//!   ← recv:  `RXHEX <hex>\n`
//!   ← log:   `LOG ...\n`
//!
//! The Python side talks to a real radio (serial/TCP/BLE) when `meshtastic` is installed.

use crate::frame::{decode_frame, encode_frame, Frame, MsgType};
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

pub struct MeshtasticStdioTransport {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl MeshtasticStdioTransport {
    /// Spawn bridge script. `script` path to meshtastic_bridge.py; `port` e.g. /dev/ttyUSB0 or "tcp:localhost:4403"
    pub fn spawn(script: &str, port: &str, channel_index: u8) -> Result<Self> {
        let mut child = Command::new("python3")
            .arg(script)
            .arg("--port")
            .arg(port)
            .arg("--channel-index")
            .arg(channel_index.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("spawn meshtastic_bridge.py")?;

        let stdout = child.stdout.take().context("stdout")?;
        let stdin = child.stdin.take().context("stdin")?;
        let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let _ = tx.send(line);
            }
        });

        Ok(Self { child, stdin, rx })
    }

    pub fn send_raw(&mut self, frame_bytes: &[u8]) -> Result<()> {
        let hex = hex::encode(frame_bytes);
        writeln!(self.stdin, "TXHEX {hex}")?;
        self.stdin.flush()?;
        Ok(())
    }

    pub fn send_frame(&mut self, msg_type: MsgType, payload: &[u8]) -> Result<()> {
        let bytes = encode_frame(msg_type, payload).map_err(|e| anyhow::anyhow!(e))?;
        self.send_raw(&bytes)
    }

    /// Non-blocking poll for one frame.
    pub fn try_recv_frame(&self) -> Result<Option<Frame>> {
        match self.rx.try_recv() {
            Ok(line) => {
                if let Some(rest) = line.strip_prefix("RXHEX ") {
                    let bytes = hex::decode(rest.trim()).context("hex decode")?;
                    let frame = decode_frame(&bytes).map_err(|e| anyhow::anyhow!(e))?;
                    Ok(Some(frame))
                } else if line.starts_with("LOG ") || line.starts_with("OK") {
                    Ok(None)
                } else if line.starts_with("ERR ") {
                    bail!("bridge error: {line}");
                } else {
                    Ok(None)
                }
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => bail!("bridge process disconnected"),
        }
    }

    pub fn shutdown(mut self) -> Result<()> {
        let _ = writeln!(self.stdin, "QUIT");
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for MeshtasticStdioTransport {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
