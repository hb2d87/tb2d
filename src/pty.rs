use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

pub type PaneId = usize;

#[derive(Debug)]
pub enum PtyEvent {
    Output(PaneId, Vec<u8>),
    Exited(PaneId),
    ReadError(PaneId, String),
}

struct PtyHandle {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct PtyManager {
    panes: HashMap<PaneId, PtyHandle>,
    tx: Sender<PtyEvent>,
    rx: Receiver<PtyEvent>,
}

impl PtyManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { panes: HashMap::new(), tx, rx }
    }

    pub fn spawn(&mut self, id: PaneId, command: &str) -> Result<()> {
        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .context("failed to open PTY")?;
        let mut builder = CommandBuilder::new("sh");
        builder.arg("-lc");
        builder.arg(command);
        let child = pair.slave.spawn_command(builder)
            .with_context(|| format!("failed to spawn pane command {command:?}"))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().context("failed to clone PTY reader")?;
        let writer = pair.master.take_writer().context("failed to open PTY writer")?;
        let tx = self.tx.clone();
        thread::spawn(move || read_output(id, &mut reader, tx));
        self.panes.insert(id, PtyHandle { master: pair.master, writer, child });
        Ok(())
    }

    pub fn write(&mut self, id: PaneId, input: &[u8]) -> Result<()> {
        if let Some(pane) = self.panes.get_mut(&id) {
            pane.writer.write_all(input).context("failed to write PTY input")?;
            pane.writer.flush().context("failed to flush PTY input")?;
        }
        Ok(())
    }

    pub fn resize(&self, id: PaneId, cols: u16, rows: u16) -> Result<()> {
        if let Some(pane) = self.panes.get(&id) {
            pane.master
                .resize(PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 })
                .context("failed to resize PTY")?;
        }
        Ok(())
    }

    pub fn try_recv(&self) -> Option<PtyEvent> {
        self.rx.try_recv().ok()
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PtyManager {
    fn drop(&mut self) {
        for pane in self.panes.values_mut() {
            let _ = pane.child.kill();
        }
    }
}

fn read_output(id: PaneId, reader: &mut dyn Read, tx: Sender<PtyEvent>) {
    let mut buffer = [0_u8; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                let _ = tx.send(PtyEvent::Exited(id));
                break;
            }
            Ok(count) => {
                if tx.send(PtyEvent::Output(id, buffer[..count].to_vec())).is_err() {
                    break;
                }
            }
            Err(error) => {
                let _ = tx.send(PtyEvent::ReadError(id, error.to_string()));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn spawns_command_and_captures_output() {
        let mut manager = PtyManager::new();
        manager.spawn(7, "printf tb2d-pty-ok").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Some(PtyEvent::Output(7, output)) = manager.try_recv() {
                assert!(String::from_utf8_lossy(&output).contains("tb2d-pty-ok"));
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("PTY output was not received");
    }
}
