extern crate kqueue_sys;
extern crate libc;
extern crate walkdir;

use kqueue_sys::constants::EventFilter::*;
use kqueue_sys::constants::*;
use kqueue_sys::*;
use walkdir::{DirEntry, WalkDir};

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};

struct DirWatcher {
    kq: libc::c_int,
    fd_to_path: HashMap<RawFd, PathBuf>,
    path_to_fd: BTreeMap<PathBuf, RawFd>,
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
            path_to_fd: BTreeMap::new(),
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
                fflags: NOTE_DELETE | NOTE_WRITE | NOTE_EXTEND | NOTE_LINK | NOTE_RENAME,
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

    fn add_dir<P: AsRef<Path>>(&mut self, path: P) -> usize {
        let mut added = 0;
        WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| self.add(entry.path()))
            .count()
    }

    fn remove_dir<P: AsRef<Path>>(&mut self, path: P) -> usize {
        let to_remove = self
            .path_to_fd
            .range(path.as_ref().to_path_buf()..)
            .map(|(p, _fd)| p.clone())
            .take_while(|p| p.starts_with(&path))
            .collect::<Vec<PathBuf>>();

        to_remove.iter().filter(|entry| self.remove(entry)).count()
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

        let fd = ev.ident as i32;
        self.fd_to_path
            .get(&fd)
            .map(|path| (path.as_ref(), ev.fflags))
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
    let added = watcher.add_dir("test");
    println!("Added {}", added);
    println!("Event: {:?}", watcher.next_event());
    let removed = watcher.remove_dir("test");
    println!("Removed {}", removed);

    // for entry in WalkDir::new("test")
    //     .into_iter()
    //     .filter_map(|entry| entry.ok())
    // {
    //     if watcher.add(entry.path()) {
    //         println!("Watch: {}", entry.path().display());
    //     }
    // }

    // while let Some((path, flags)) = watcher.next_event() {
    //     println!("Event {:?} on {}", flags, path.display());
    // }

    Ok(())
}
