use bytesize::ByteSize;
use indicatif::ProgressBar;
use std::{
    borrow::Cow,
    collections::HashMap,
    env::args,
    ops::Not,
    process,
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
    time::Duration,
};
use walkdir::WalkDir;

static FINISHED: AtomicBool = AtomicBool::new(false);

struct FolderStat {
    size: u64,
    files: usize,
}

fn show_stats(folder_sizes: HashMap<String, FolderStat>) {
    let total = folder_sizes
        .iter()
        .map(|(_, stat)| stat.size)
        .max()
        .unwrap_or_default();

    let mut result = Vec::from_iter(folder_sizes);
    result.sort_unstable_by_key(|(_, stat)| stat.size);

    println!("{:>12} {:>7} {:>10} Folder Name", "Size", "dist.", "Files");
    for (name, fs) in result {
        println!(
            "{:>12} {:>6.2}% {:>10} {name} ",
            ByteSize(fs.size),
            (fs.size as f64 / total as f64) * 100.,
            fs.files
        );
    }
}

fn check_show_help(root_path: &str) {
    if root_path == "--help" || root_path == "-h" {
        println!("Usage:\n\tf-stats <folder-name>\n");
        process::exit(0);
    }
}

fn start_ticker(progress: ProgressBar) -> JoinHandle<()> {
    thread::spawn(move || {
        while FINISHED.load(Ordering::Acquire).not() {
            thread::sleep(Duration::from_millis(500));
            progress.tick();
        }
    })
}

fn scan_folders(root_path: &str) -> HashMap<String, FolderStat> {
    let bar = ProgressBar::new_spinner();
    let ticker = start_ticker(bar.clone());
    let mut folder_sizes = HashMap::new();
    for entry in WalkDir::new(root_path).into_iter().flat_map(|f| f.ok()) {
        if entry.path().is_dir() && entry.depth() < 4 {
            let folder_name = entry
                .path()
                .to_str()
                .map(|s| Cow::Borrowed(s))
                .unwrap_or_else(|| entry.path().to_string_lossy());
            bar.set_message(format!("Scanning: {folder_name}"));
        }

        if let Ok(size) = entry.metadata().map(|md| md.len()) {
            for p in entry
                .path()
                .ancestors()
                .skip(entry.depth().checked_sub(1).unwrap_or(1))
                .filter_map(|p| {
                    p.as_os_str()
                        .is_empty()
                        .not()
                        .then_some(p.as_os_str())
                        .and_then(|s| s.to_str())
                })
                .map(String::from)
            {
                folder_sizes
                    .entry(p)
                    .and_modify(|fs: &mut FolderStat| {
                        fs.size += size;
                        fs.files += 1;
                    })
                    .or_insert(FolderStat { size, files: 1 });
            }
        }
    }
    bar.set_message(format!("Completed in {} ms", bar.elapsed().as_millis()));
    FINISHED.store(true, Ordering::Release);
    ticker.join().unwrap();
    bar.finish();
    folder_sizes
}

fn main() {
    let root_path = args().nth(1).unwrap_or_else(|| ".".into());
    check_show_help(&root_path);
    show_stats(scan_folders(&root_path));
}
