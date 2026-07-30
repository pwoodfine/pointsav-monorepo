use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

/// Recursive inotify-based filesystem watcher using raw Linux syscalls.
/// Sovereign replacement for the `notify` crate.
/// Caller receives path events on `rx`; bridge to tokio via `spawn_blocking` + channel.
pub struct FsWatcher {
    pub rx: mpsc::Receiver<PathBuf>,
}

impl FsWatcher {
    pub fn watch(path: &Path) -> Self {
        let (tx, rx) = mpsc::channel();
        let root = path.to_path_buf();
        thread::spawn(move || run_watcher(root, tx));
        FsWatcher { rx }
    }
}

fn add_watch(fd: RawFd, path: &Path) -> Option<i32> {
    let bytes = path.to_string_lossy();
    let c_path = CString::new(bytes.as_bytes()).ok()?;
    let wd = unsafe {
        libc::inotify_add_watch(
            fd,
            c_path.as_ptr(),
            libc::IN_MODIFY
                | libc::IN_CREATE
                | libc::IN_DELETE
                | libc::IN_MOVED_FROM
                | libc::IN_MOVED_TO,
        )
    };
    if wd < 0 {
        None
    } else {
        Some(wd)
    }
}

fn add_recursive(fd: RawFd, path: &Path, watches: &mut HashMap<i32, PathBuf>) {
    if let Some(wd) = add_watch(fd, path) {
        watches.insert(wd, path.to_path_buf());
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                add_recursive(fd, &entry.path(), watches);
            }
        }
    }
}

fn run_watcher(root: PathBuf, tx: mpsc::Sender<PathBuf>) {
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if fd < 0 {
        return;
    }

    let mut watches: HashMap<i32, PathBuf> = HashMap::new();
    add_recursive(fd, &root, &mut watches);

    let event_size = std::mem::size_of::<libc::inotify_event>();
    let buf_size = event_size * 64 + 4096;
    let mut buf: Vec<u8> = vec![0u8; buf_size];

    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf_size) };
        if n <= 0 {
            break;
        }
        let n = n as usize;
        let mut offset = 0usize;

        while offset + event_size <= n {
            let event = unsafe { &*(buf.as_ptr().add(offset) as *const libc::inotify_event) };
            let name_len = event.len as usize;
            let dir = watches
                .get(&event.wd)
                .cloned()
                .unwrap_or_else(|| root.clone());

            let event_path = if name_len > 0 && offset + event_size + name_len <= n {
                let name_ptr = unsafe { buf.as_ptr().add(offset + event_size) };
                let name_bytes = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };
                let nul = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_len);
                if let Ok(name) = std::str::from_utf8(&name_bytes[..nul]) {
                    dir.join(name)
                } else {
                    dir.clone()
                }
            } else {
                dir.clone()
            };

            // Auto-watch newly created subdirectories
            if event.mask & libc::IN_CREATE != 0 && event.mask & libc::IN_ISDIR != 0 {
                if let Some(wd) = add_watch(fd, &event_path) {
                    watches.insert(wd, event_path.clone());
                }
            }

            if tx.send(event_path).is_err() {
                unsafe { libc::close(fd) };
                return;
            }

            offset += event_size + name_len;
        }
    }

    unsafe { libc::close(fd) };
}

pub fn system_status() -> &'static str {
    "moonshot-fs-watch: active (inotify)"
}
