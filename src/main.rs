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
use std::path::{Path, PathBuf};

use std::os::unix::io::{IntoRawFd, RawFd};

/// A very simple kevent-driven directory watcher.
///
/// Unfortunately kevent is quite expensive for watching file trees, costing an
/// fd for each file and directory to be monitored.  Rename detection is handled
/// by re-scanning the base monitoring directory.  Thus this should only be used
/// on relatively small trees with relatively few renames.

struct KEventDir {
    kq: libc::c_int,
    basedir: PathBuf,
    fd_to_path: HashMap<RawFd, PathBuf>,
    path_to_fd: BTreeMap<PathBuf, RawFd>,
}

impl KEventDir {
    fn new<P: AsRef<Path>>(basedir: P) -> Option<Self> {
        let kq = unsafe { kqueue() };
        if kq < 0 {
            return None;
        }

        Some(Self {
            kq,
            basedir: basedir.as_ref().to_owned(),
            fd_to_path: HashMap::new(),
            path_to_fd: BTreeMap::new(),
        })
    }

    fn add_base(&mut self) -> usize {
        let base = self.basedir.to_owned();
        self.add_dir(&base)
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
            unsafe { libc::close(fd) };
            true
        } else {
            false
        }
    }

    fn add_dir<P: AsRef<Path>>(&mut self, path: P) -> usize {
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

    fn close(self) {
        drop(self);
    }
}

impl Iterator for KEventDir {
    type Item = (PathBuf, FilterFlag);

    fn next(&mut self) -> Option<Self::Item> {
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

        let path = self.fd_to_path.get(&fd).map(|p| p.to_owned())?;
        if ev.fflags & NOTE_DELETE == NOTE_DELETE {
            self.remove(&path);
        }

        if ev.fflags & NOTE_RENAME == NOTE_RENAME {
            self.remove_dir(&path);
            self.add_base();
        }

        if ev.fflags & NOTE_WRITE == NOTE_WRITE {
            self.add_dir(&path);
        }

        Some((path, ev.fflags))
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

fn main() -> Result<(), String> {
    let mut watcher = KEventDir::new("test").unwrap();
    let added = watcher.add_base();
    println!("Added {}", added);

    watcher
        .by_ref()
        .take(20)
        .for_each(|(path, flags)| println!("{}: {:?}", path.display(), flags));

    //println!("Event: {:?}", watcher.next());
    let removed = watcher.remove_dir("test");
    println!("Removed {}", removed);

    watcher.close();
    Ok(())
}
