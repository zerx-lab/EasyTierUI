//! One running app per user. The first instance holds an exclusive lock on `instance.lock` in the app's data
//! directory and listens on a loopback port it writes to `instance.port`; a later launch finds the lock taken, asks the
//! first one to show its window, and leaves. The listener only understands that one request, so another local process
//! can at most bring the window to the front. The OS drops the lock when the first instance ends, however it ends.

use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use crate::{ShellError, platform, shell::Action};

const LOCK_FILE: &str = "instance.lock";
const PORT_FILE: &str = "instance.port";
/// The only request: show the window.
const SHOW: &[u8] = b"show\n";
/// How long either side waits on the other.
const TIMEOUT: Duration = Duration::from_secs(1);

/// What claiming the instance found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// This process is the app's one instance (or already was).
    First,
    /// Another instance runs; it was asked to show its window. This one should quit.
    Second,
}

/// The lock of this process's claim, held until the process ends.
static HELD: OnceLock<File> = OnceLock::new();

/// Claims the single instance in `dir`. Repeated calls in the process that holds it answer [`Claim::First`] (several
/// app roots in one test process share it).
///
/// # Errors
///
/// [`ShellError::Io`] when the lock file cannot be opened or locked, [`ShellError::Listener`] when the first instance's
/// listener cannot be set up. A second instance whose request does not get through still answers [`Claim::Second`]:
/// the first may be starting, and a duplicate would run the same networks twice.
pub fn claim(dir: &Path) -> Result<Claim, ShellError> {
    if HELD.get().is_some() {
        return Ok(Claim::First);
    }
    let lock_path = dir.join(LOCK_FILE);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(io_error("open", &lock_path))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            ask_first_to_show(&dir.join(PORT_FILE));
            return Ok(Claim::Second);
        }
        Err(TryLockError::Error(source)) => return Err(io_error("lock", &lock_path)(source)),
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(ShellError::Listener)?;
    let port = listener.local_addr().map_err(ShellError::Listener)?.port();
    let port_path = dir.join(PORT_FILE);
    write_port(&port_path, port).map_err(io_error("write", &port_path))?;
    std::thread::Builder::new()
        .name("easytierui-instance".into())
        .spawn(move || listen(&listener))
        .map_err(ShellError::Listener)?;
    if HELD.set(lock).is_err() {
        log::warn!("the instance was claimed twice at once");
    }
    Ok(Claim::First)
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> ShellError {
    let path: PathBuf = path.to_owned();
    move |source| ShellError::Io {
        action,
        path,
        source,
    }
}

/// Writes the port beside the lock, replacing the file so a reader never sees half of it.
fn write_port(path: &Path, port: u16) -> std::io::Result<()> {
    let temporary = path.with_extension("port.tmp");
    fs::write(&temporary, port.to_string())?;
    fs::rename(&temporary, path)
}

/// Shows the window on each `show` request.
fn listen(listener: &TcpListener) {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                log::warn!("the instance listener: {error}");
                continue;
            }
        };
        if let Err(error) = stream.set_read_timeout(Some(TIMEOUT)) {
            log::warn!("the instance listener: {error}");
            continue;
        }
        let mut request = [0_u8; SHOW.len()];
        match stream.read_exact(&mut request) {
            Ok(()) if request == SHOW => platform::request(Action::Show),
            Ok(()) => log::warn!("the instance listener ignored an unknown request"),
            Err(error) => log::debug!("the instance listener: {error}"),
        }
    }
}

/// Asks the instance listening on the port in `port_path` to show its window.
fn ask_first_to_show(port_path: &Path) {
    platform::allow_foreground();
    let port = match fs::read_to_string(port_path) {
        Ok(text) => text.trim().parse::<u16>().ok(),
        Err(error) => {
            log::warn!("{}: {error}", port_path.display());
            None
        }
    };
    let Some(port) = port else {
        log::warn!("the running instance's port is unknown; it may still be starting");
        return;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let sent = TcpStream::connect_timeout(&address, TIMEOUT).and_then(|mut stream| {
        stream.set_write_timeout(Some(TIMEOUT))?;
        stream.write_all(SHOW)
    });
    if let Err(error) = sent {
        log::warn!("the running instance did not take the request to show its window: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_finds_the_first_and_sends_show() {
        let dir =
            std::env::temp_dir().join(format!("desktop-shell-instance-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // The first instance: another holder of the lock, with a listener of the test's own.
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(LOCK_FILE))
            .unwrap();
        held.lock().unwrap();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        write_port(&dir.join(PORT_FILE), listener.local_addr().unwrap().port()).unwrap();

        assert_eq!(claim(&dir).unwrap(), Claim::Second);
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        stream.read_to_end(&mut request).unwrap();
        assert_eq!(request, SHOW);
        assert!(HELD.get().is_none(), "a second instance holds nothing");

        drop(held);
        fs::remove_dir_all(&dir).unwrap();
    }
}
