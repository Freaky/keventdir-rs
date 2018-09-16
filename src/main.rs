extern crate keventdir;

use keventdir::KEventDir;
use std::ffi::OsString;
use std::time::Duration;

fn main() -> Result<(), String> {
    let args = std::env::args_os().skip(1).collect::<Vec<OsString>>();
    let mut watcher = KEventDir::new().expect("kqueue");
    for arg in args.iter() {
        watcher.add_recursive_rescan(arg);
    }
    println!("Monitoring {} descriptors", watcher.rescan());

    // Poll kevent() with an optional duration
    if let Some(ev) = watcher.poll(Some(Duration::from_secs(1))) {
        let ev = ev.unwrap();
        println!("{}: {:?}", ev.path.display(), ev.kind)
    } else {
        println!("No event in 1 second");
    }

    // Or use as an Iterator with an unlimited delay between events
    watcher
        .by_ref()
        .take(20)
        .map(|ev| ev.unwrap())
        .for_each(|ev| println!("{}: {:?}", ev.path.display(), ev.kind));

    // For demo purposes, normally you'd just let it drop out of scope.
    let removed = args
        .iter()
        .map(|arg| watcher.remove_recursive(arg))
        .sum::<usize>();
    println!("Dropped {} descriptors", removed);

    // equivalent to drop(watcher)
    watcher.close();
    Ok(())
}
