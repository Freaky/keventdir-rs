extern crate kqueue_sys;
extern crate walkdir;

use kqueue_sys::constants::*;
use kqueue_sys::constants::EventFilter::*;
// use kqueue_sys::constants::EventFlag::*;
// use kqueue_sys::constants::FilterFlag::*;
use kqueue_sys::*;
use walkdir::{DirEntry, WalkDir};

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use std::os::unix::io::{IntoRawFd, RawFd};

fn main() -> Result<(), String> {
    let watching = WalkDir::new("test")
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            File::open(&entry.path())
                .map(|fd| (fd.into_raw_fd(), entry.path().to_owned()))
                .ok()
        }).collect::<HashMap<RawFd, PathBuf>>();

    println!("{:?}", watching);

    let kq = unsafe { kqueue() };

    if kq == -1 {
        return Err("kqueue".into());
    }

    for (fd, path) in &watching {
        let mut event = kevent {
            ident: *fd as usize,
            filter: EVFILT_VNODE,
            flags: EV_ADD | EV_CLEAR,
            fflags: NOTE_DELETE | NOTE_WRITE | NOTE_EXTEND | NOTE_RENAME | NOTE_CLOSE_WRITE,
            data: 0,
            udata: std::ptr::null_mut()
        };
        unsafe {
            let v = kevent(kq, &mut event, 1, std::ptr::null_mut(), 0, std::ptr::null());
            if v == -1 {
                return Err("kevent register".into());
            }
        }
    }

    loop {
        let mut ev = kevent {
            ident: 0,
            filter: EVFILT_VNODE,
            flags: EV_ADD,
            fflags: NOTE_FFNOP,
            data: 0,
            udata: std::ptr::null_mut()
        };
        let ret = unsafe { kevent(kq, std::ptr::null_mut(), 0, &mut ev, 1, std::ptr::null()) };

        if ret == -1 {
            return Err("kevent watch".into());
        }

        let fd = ev.ident as i32;

        println!("event on: {}", watching.get(&fd).map(|path| path.display()).unwrap());
    }

    Ok(())
}
