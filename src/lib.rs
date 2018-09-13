extern crate kqueue_sys;
extern crate libc;
extern crate walkdir;

use kqueue_sys::constants::EventFilter::*;
use kqueue_sys::constants::*;
use kqueue_sys::*;
use walkdir::WalkDir;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::os::unix::io::{IntoRawFd, RawFd};
use std::path::{Path, PathBuf};

/// A very simple kevent-driven directory watcher.
///
/// Unfortunately kevent is quite expensive for watching file trees, costing an
/// fd for each file and directory to be monitored.  Rename detection is handled
/// by re-scanning the base monitoring directory.  Thus this should only be used
/// on relatively small trees with relatively few renames.

#[derive(Debug)]
pub struct KEventDir {
    kq: libc::c_int,
    basedir: PathBuf,
    fd_to_path: HashMap<RawFd, PathBuf>,
    path_to_fd: BTreeMap<PathBuf, RawFd>,
}

#[derive(Debug)]
pub enum EventType {
    Delete,
    Extend,
    Link,
    Other,
    Rename,
    Revoke,
    Write,
}

impl KEventDir {
    pub fn new<P: AsRef<Path>>(basedir: P) -> io::Result<Self> {
        let kq = unsafe { kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Self {
            kq,
            basedir: basedir.as_ref().to_owned(),
            fd_to_path: HashMap::new(),
            path_to_fd: BTreeMap::new(),
        })
    }

    pub fn add_base(&mut self) -> usize {
        let base = self.basedir.to_owned();
        self.add_dir(&base)
    }

    pub fn add<P: AsRef<Path>>(&mut self, path: P) -> io::Result<bool> {
        let path = path.as_ref();

        if self.path_to_fd.contains_key(path) {
            return Ok(false);
        }

        File::open(path).and_then(|file| {
            let fd = file.into_raw_fd();
            let event = kevent {
                ident: fd as usize,
                filter: EVFILT_VNODE,
                flags: EV_ADD | EV_CLEAR,
                fflags: NOTE_DELETE
                    | NOTE_WRITE
                    | NOTE_EXTEND
                    | NOTE_LINK
                    | NOTE_REVOKE
                    | NOTE_RENAME,
                data: 0,
                udata: std::ptr::null_mut(),
            };

            let v = unsafe {
                kevent(
                    self.kq,
                    &event,
                    1,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };

            if v != -1 {
                self.fd_to_path.insert(fd, path.to_owned());
                self.path_to_fd.insert(path.to_owned(), fd);
                Ok(true)
            } else {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                Err(err)
            }
        })
    }

    pub fn remove<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let path = path.as_ref();
        if let Some(fd) = self.path_to_fd.remove(path) {
            self.fd_to_path.remove(&fd);
            // last close deletes the event automatically
            unsafe { libc::close(fd) };
            true
        } else {
            false
        }
    }

    pub fn add_dir<P: AsRef<Path>>(&mut self, path: P) -> usize {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| self.add(entry.path()).unwrap_or(false))
            .count()
    }

    pub fn remove_dir<P: AsRef<Path>>(&mut self, path: P) -> usize {
        let to_remove = self
            .path_to_fd
            .range(path.as_ref().to_path_buf()..)
            .map(|(p, _fd)| p)
            .take_while(|p| p.starts_with(&path))
            .cloned()
            .collect::<Vec<PathBuf>>();

        to_remove.iter().filter(|entry| self.remove(entry)).count()
    }

    pub fn close(self) {
        drop(self);
    }
}

impl Iterator for KEventDir {
    type Item = io::Result<(PathBuf, EventType)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ev = kevent {
            ident: 0,
            filter: EVFILT_VNODE,
            flags: EV_ADD,
            fflags: NOTE_FFNOP,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        loop {
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
                let err = io::Error::last_os_error();
                if err.kind() != io::ErrorKind::Interrupted {
                    return Some(Err(err));
                }
            } else {
                break;
            }
        }

        let fd = ev.ident as i32;

        let path = self.fd_to_path.get(&fd).map(|p| p.to_owned())?;

        let kind = if ev.fflags.contains(NOTE_DELETE) {
            self.remove(&path);
            EventType::Delete
        } else if ev.fflags.contains(NOTE_REVOKE) {
            self.remove_dir(&path);
            EventType::Revoke
        } else if ev.fflags.contains(NOTE_RENAME) {
            self.remove_dir(&path);
            self.add_base();
            EventType::Rename
        } else if ev.fflags.contains(NOTE_LINK) {
            self.add_dir(&path);
            EventType::Link
        } else if ev.fflags.contains(NOTE_WRITE) {
            self.add_dir(&path);
            EventType::Write
        } else {
            EventType::Other
        };

        Some(Ok((path, kind)))
    }
}

impl Drop for KEventDir {
    fn drop(&mut self) {
        for fd in self.fd_to_path.keys() {
            unsafe { libc::close(*fd) };
        }

        unsafe { libc::close(self.kq) };
    }
}
