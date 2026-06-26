//! Embedded PTY terminal. Spawns a shell through a pseudo-terminal (ConPTY on
//! Windows) and bridges it to the frontend's xterm.js: bytes read from the PTY
//! are emitted as `term-output` events; the frontend sends keystrokes via
//! `term_write`. One session at a time (replaced on re-open).
//!
//! Design notes:
//! - The writer lives behind its OWN lock so a slow/blocking write never stalls
//!   `resize`/`close` (the UI's recovery path).
//! - Each session has a monotonic `epoch`; output/exit events carry it so a
//!   stale reader thread from a replaced session can't bleed into the new one.

use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

struct Session {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
}

/// Tauri-managed state for the (single) active terminal session.
#[derive(Default)]
pub struct TermState {
    session: Mutex<Option<Session>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    epoch: AtomicU64,
}

#[derive(Clone, serde::Serialize)]
struct Output {
    epoch: u64,
    data: Vec<u8>,
}

#[derive(Clone, serde::Serialize)]
struct ExitMsg {
    epoch: u64,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Start `shell` in a PTY at `cwd`, replacing any prior session. Returns the new
/// session epoch (so the frontend can ignore stragglers from an old session).
pub fn open(
    app: &AppHandle,
    state: &TermState,
    shell: &str,
    cwd: &str,
    cols: u16,
    rows: u16,
) -> Result<u64, String> {
    close(state);
    let epoch = state.epoch.fetch_add(1, Ordering::SeqCst) + 1;

    let pty = native_pty_system();
    let pair = pty.openpty(size(cols, rows)).map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::new(shell);
    if !cwd.trim().is_empty() {
        cmd.cwd(cwd);
    }
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    // Drop the slave in the parent so the reader sees EOF when the child exits.
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    *state.session.lock() = Some(Session {
        master: pair.master,
        child,
    });
    *state.writer.lock() = Some(writer);

    let app2 = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let msg = Output { epoch, data: buf[..n].to_vec() };
                    if app2.emit("term-output", msg).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = app2.emit("term-exit", ExitMsg { epoch });
    });
    Ok(epoch)
}

pub fn write(state: &TermState, data: &str) -> Result<(), String> {
    // Only the writer lock is held here, so a blocking write can't stall resize/close.
    let mut guard = state.writer.lock();
    if let Some(w) = guard.as_mut() {
        w.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn resize(state: &TermState, cols: u16, rows: u16) -> Result<(), String> {
    let guard = state.session.lock();
    if let Some(s) = guard.as_ref() {
        s.master.resize(size(cols, rows)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn close(state: &TermState) {
    // Kill the child first — this breaks the pipe and unblocks any in-flight
    // write — then release the writer.
    if let Some(mut s) = state.session.lock().take() {
        let _ = s.child.kill();
    }
    *state.writer.lock() = None;
}
