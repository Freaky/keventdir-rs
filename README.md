# KEventDir

A simple kevent-driven directory watcher for Rust.

## Synopsis

```rust
extern crate keventdir;

use keventdir::KEventDir;

fn main() {
    let watcher = KEventDir::new("content").expect("kqueue");
    watcher.add_base(); // scan some_dir and add to kevent: watcher is inert otherwise

    // rename detection is only partial outside the base directory: old files are
    // removed but only the base directory is checked for new ones.
    watcher.add("config.toml"); // watch this one file
    watcher.add_dir("static"); // watch this directory tree

    for (path, event) in watcher.by_ref().take(10) {
        println!("{} changed: {:?}", path.display(), event);
    }
}
```

Effort is made to track file creation, renames and deletes.  Additional files
and directories can be added using `add()` and `add_dir()`, the latter being
recursive.  `remove()` and `remove_dir()` work similarly.  Entries already
being monitored will be skipped.

Renames will currently only show the old filename, new filenames can potentially
be detected on a best-effort basis in principle, but it's not yet implemented.
New files will be added automatically.

## Status

This is just a quick proof-of-concept.  Use [notify](https://github.com/passcod/notify)
if you need to use something production-ready and portable.
