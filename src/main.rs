extern crate keventdir;

use keventdir::KEventDir;
use std::ffi::OsString;

fn main() -> Result<(), String> {
    let args = std::env::args_os().skip(1).collect::<Vec<OsString>>();
    let mut watcher = KEventDir::new().expect("kqueue");
    args.iter().for_each(|arg| { watcher.add_recursive_rescan(arg); });
    println!("Monitoring {} descriptors", watcher.rescan());

    watcher
        .by_ref()
        .take(20)
        .map(|ev| ev.unwrap())
        .for_each(|ev| println!("{}: {:?}", ev.path.display(), ev.kind));

    let removed = args
        .iter()
        .map(|arg| watcher.remove_recursive(arg))
        .sum::<usize>();
    println!("Dropped {} descriptors", removed);

    watcher.close();
    Ok(())
}
