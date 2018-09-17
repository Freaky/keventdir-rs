# KEventDir

A simple kevent-driven directory watcher for Rust.

## Synopsis

Currently more a proof-of-concept than a production-ready crate:

```rust
extern crate keventdir;

use keventdir::KEventDir;

fn main() {
    let watcher = KEventDir::new().expect("kqueue");
    watcher.add_recursive_rescan("content"); // rescan this dir on file rename
    watcher.rescan(); // actually add that directory, returning number of new files

    // rename detection is only partial outside the base directory: old files are
    // removed but only the base directory is checked for new ones.
    watcher.add("config.toml").expect("returns io::Result"); // watch this one file
    watcher.add_recursive("static"); // watch this directory tree, returns number added

    // KEventDir implements Iterator over io::Result<keventdir::Event>
    for ev in watcher.by_ref().filter_map(|ev| ev.ok()).take(10) {
        println!("{}: {:?}", ev.path.display(), ev.kind);
    }

    // You can also poll for events, either non-blocking or with a time limit
    if let Some(ev) = watcher.poll(Some(Duration::from_secs(20))) {
        let ev = ev.expect("kevent");
        println!("{}: {:?}", ev.path.display(), ev.kind)
    }
}
```

Commands:

```
-% echo "meep" >>content/meep
-% rm content/meep
-% echo "woof" >content/dogs
-% mv content/{dogs,cats}
-% echo "meow" >>content/cats
-% mv content/{cats,dogs}
-% mkdir content/bam
-% rmdir content/bam
```

Output:

```
content: Write
content: Write
content/meep: Delete
content: Write
content: Write
content/dogs: Rename
content/cats: Write
content: Write
content/cats: Rename
content: Link
content: Link
content/bam: Delete
```

I'm still pinning down a sensible API.  Renames and new file handling is less
than ideal, and it probably needs to be broken up into higher and lower-level
interfaces, one with basic efficient kqueue stuff and one with debounced events
and more expensive stuff like adding new files on rename.

## Status

This is just a quick proof-of-concept.  You might find it useful or interesting,
but if it breaks you get to keep all the pieces.

Use [notify](https://github.com/passcod/notify) if you need to use something
production-ready and portable, though note it currently uses polling on BSD's:
i.e. it does a `walkdir` every few seconds.

## Caveats

kevent isn't really designed with monitoring large numbers of files in mind.
Each and every file and directory to be monitored needs to be opened, and kept
open, because it operates on file descriptors, not file names.

For most typical use it's probably fine, but don't go adding `/` to it.
