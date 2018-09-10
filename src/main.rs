extern crate kqueue_sys;
extern crate libc;
extern crate walkdir;

use kqueue::*;
use kqueue_sys::constants::EventFilter::*;
use kqueue_sys::constants::*;
use walkdir::{DirEntry, WalkDir};

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

struct DirWatcher {
    kq: libc::c_int,
    fd_to_path: HashMap<RawFd, PathBuf>,
    path_to_fd: HashMap<PathBuf, RawFd>,
}

impl DirWatcher {
    fn new() -> Option<Self> {
        let kq = unsafe { kqueue() };
        if kq < 0 {
            return None;
        }

        Some(Self {
            kq,
            fd_to_path: HashMap::new(),
            path_to_fd: HashMap::new(),
        })
    }

    fn add<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path = path.as_ref();
        if self.path_to_fd.contains_key(path) {
            return false;
        }

        if let Ok(fd) = File::open(path).map(|fd| fd.into_raw_fd()) {
            let mut event = kevent {
                ident: fd as usize,
                filter: EVFILT_VNODE,
                flags: EV_ADD | EV_CLEAR,
                fflags: NOTE_DELETE
                    | NOTE_WRITE
                    | NOTE_EXTEND
                    | NOTE_LINK
                    | NOTE_RENAME
                    | NOTE_CLOSE_WRITE,
                data: 0,
                udata: std::ptr::null_mut(),
            };

            let v = unsafe {
                kevent(
                    self.kq,
                    &mut event,
                    1,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
            if v != -1 {
                self.fd_to_path.insert(fd, path.to_owned());
                self.path_to_fd.insert(path.to_owned(), fd);
                return true;
            }
        }

        false
    }

    fn remove<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path = path.as_ref();
        if let Some(fd) = self.path_to_fd.remove(path) {
            self.fd_to_path.remove(&fd);
            // last close deletes the event automatically
            unsafe { File::from_raw_fd(fd) };
            true
        } else {
            false
        }
    }

    fn next_event(&mut self) -> Option<(&Path, FilterFlag)> {
        let mut ev = kevent {
            ident: 0,
            filter: EVFILT_VNODE,
            flags: EV_ADD,
            fflags: NOTE_FFNOP,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        let ret = unsafe {
            kevent(
                self.kq,
                std::ptr::null_mut(),
                0,
                &mut ev,
                1,
                std::ptr::null(),
            )
        };

        if ret == -1 {
            return None;
        }

        // println!("{:?}", ev);

        let fd = ev.ident as i32;
        Some((&self.fd_to_path[&fd], ev.fflags))

        // println!("event on: {}", watching.get(&fd).map(|path| path.display()).unwrap());
    }

    fn close(mut self) {
        drop(&mut self);
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        for (fd, _path) in &self.fd_to_path {
            unsafe { File::from_raw_fd(*fd) };
        }
    }
}

fn main() -> Result<(), String> {
    let mut watcher = DirWatcher::new().unwrap();

    for entry in WalkDir::new("test").into_iter() {
        if let Ok(entry) = entry {
            if watcher.add(entry.path()) {
                println!("Watch: {}", entry.path().display());
            }
        }
    }

    while let Some((path, flags)) = watcher.next_event() {
        println!("Event {:?} on {}", flags, path.display());
    }

    Ok(())
}
