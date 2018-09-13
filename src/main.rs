extern crate keventdir;

use keventdir::KEventDir;
use std::ffi::OsString;

fn main() -> Result<(), String> {
    let args = std::env::args_os().skip(1).collect::<Vec<OsString>>();
    let mut watcher = KEventDir::new(args.get(0).expect("need an argument")).unwrap();
    let mut added = watcher.add_base();
    added += args
        .iter()
        .skip(1)
        .map(|arg| watcher.add_dir(arg))
        .sum::<usize>();
    println!("Monitoring {} descriptors", added);

    watcher
        .by_ref()
        .take(20)
        .map(|ev| ev.unwrap())
        .for_each(|(path, flags)| println!("{}: {:?}", path.display(), flags));

    let removed = args
        .iter()
        .map(|arg| watcher.remove_dir(arg))
        .sum::<usize>();
    println!("Dropped {} descriptors", removed);

    watcher.close();
    Ok(())
}
