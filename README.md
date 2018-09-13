# KEventDir

A simple kevent-driven directory watcher for Rust.

## Synopsis

Currently more a proof-of-concept than a production-ready crate:

```rust
extern crate keventdir;

use keventdir::KEventDir;

fn main() {
    let watcher = KEventDir::new("content").expect("kqueue");
    watcher.add_base(); // scan some_dir and add to kevent: watcher is inert otherwise

    // rename detection is only partial outside the base directory: old files are
    // removed but only the base directory is checked for new ones.
    watcher.add("config.toml").expect("returns io::Result"); // watch this one file
    watcher.add_dir("static"); // watch this directory tree, returns number added

    // KEventDir implements Iterator over io::Result<keventdir::Event>
    for ev in watcher.by_ref().filter_map(|ev| ev.ok()).take(10) {
        println!("{}: {:?}", ev.path.display(), ev.kind);
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

It's a bit rough-and-ready: new files show as writes to their containing
directory, which triggers a re-scan of that directory.  New files will be added
to monitoring, but not reported as events unless they trigger some themselves
afterwards.

Similarly deletes appear as events on the file themselves, but also the
directory they're in.

Directories trigger changes in link count, hence the `Link` event.

Rename events only trigger on the original filename: monitoring will be removed
for the old name, and the base directory re-scanned in attempt to relocate the
new file.  It is not yet reported - should be doable by tracking the inode
number.

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
