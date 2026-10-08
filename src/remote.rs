//! Remote console (feature `remote-console`): a Unix socket that runs console
//! commands against the live engine, so a running game can be inspected from
//! outside. Each line sent is one command; its output comes back followed by an
//! empty line. `echo getpos | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/vibe_rm.sock`

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::engine::Engine;

type Request = (String, Sender<Vec<String>>);

pub struct RemoteConsole {
    requests: Receiver<Request>,
    path: PathBuf,
}

impl RemoteConsole {
    /// Listens at `$VRM_SOCKET`, else `$XDG_RUNTIME_DIR/vibe_rm.sock` (`/tmp` without
    /// one), or `vibe_rm-<pid>.sock` there while another instance holds that.
    pub fn start() -> std::io::Result<Self> {
        let mut path = std::env::var_os("VRM_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("XDG_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("vibe_rm.sock")
            });
        if UnixStream::connect(&path).is_ok() {
            path.set_file_name(format!("vibe_rm-{}.sock", std::process::id()));
        }
        // A socket left behind by a crashed run would make bind fail.
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        let (tx, requests) = channel();
        std::thread::Builder::new()
            .name("remote-console".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let tx = tx.clone();
                    std::thread::spawn(move || serve(stream, tx));
                }
            })?;
        log::info!("remote console listening on {}", path.display());
        Ok(RemoteConsole { requests, path })
    }

    /// Runs the commands that arrived since the last call. Call once per frame.
    pub fn poll(&self, engine: &mut Engine) {
        while let Ok((line, reply)) = self.requests.try_recv() {
            log::info!("remote console: {line}");
            let _ = reply.send(crate::console::execute(engine, &line));
        }
    }
}

impl Drop for RemoteConsole {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn serve(stream: UnixStream, tx: Sender<Request>) {
    let Ok(mut out) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (reply_tx, reply) = channel();
        if tx.send((line.to_owned(), reply_tx)).is_err() {
            return;
        }
        // The engine is gone (or exiting) if no reply comes.
        let Ok(lines) = reply.recv() else { return };
        let mut text = lines.join("\n");
        text.push_str(if lines.is_empty() { "\n" } else { "\n\n" });
        if out.write_all(text.as_bytes()).is_err() {
            return;
        }
    }
}
