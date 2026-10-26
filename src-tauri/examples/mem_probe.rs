//! K2 metadata memory probe.
//!
//! Usage: `cargo run --release --example mem_probe -- <entries>`
//!
//! Builds a synthetic metadata tree with one root, one directory per 1,000
//! files, and `<entries>` files. No database is used. Reports RSS before and
//! after, plus the cost of the hot structures and a 100,000-entry path lookup.

use kensaku_lib::indexer::arena::Node;
use kensaku_lib::indexer::metadata::MetadataStore;
use kensaku_lib::indexer::walker::FileEntry;
use std::path::PathBuf;
use std::time::Instant;

fn rss_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with(field))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0)
}

fn make_entry(path: PathBuf, is_dir: bool, id: u64) -> FileEntry {
    FileEntry {
        id,
        path: path.clone(),
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        extension: if is_dir {
            None
        } else {
            Some("txt".to_string())
        },
        size: 1,
        modified: 1,
        is_dir,
        dev: 0,
        ino: 0,
        char_bloom: 0,
        content_pending: false,
    }
}

fn main() {
    let entries = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100_000);
    let dirs = (entries / 1000).clamp(1, 100_000);

    let rss_before = rss_kb("VmRSS:");
    let hwm_before = rss_kb("VmHWM:");

    let mut store = MetadataStore::without_db();
    let mut next_id = 1u64;

    let root = PathBuf::from("/corpus");
    store.add(make_entry(root.clone(), true, next_id));
    next_id += 1;

    let mut dir_paths = Vec::with_capacity(dirs);
    for dir_index in 0..dirs {
        let dir_path = root.join(format!("d{:05}", dir_index));
        dir_paths.push(dir_path.clone());
        store.add(make_entry(dir_path, true, next_id));
        next_id += 1;
    }

    for file_index in 0..entries {
        let dir = &dir_paths[file_index % dirs];
        let path = dir.join(format!("f{:07}.txt", file_index));
        store.add(make_entry(path, false, next_id));
        next_id += 1;
    }

    let rss_after = rss_kb("VmRSS:");
    let hwm_after = rss_kb("VmHWM:");

    let mut samples = Vec::with_capacity(100_000);
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..100_000usize {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let idx = (state as usize) % store.len();
        let path = store.path(idx as u32);
        let start = Instant::now();
        let found = store.get_index(&path);
        let elapsed = start.elapsed().as_nanos();
        samples.push(elapsed);
        let _ = found;
    }
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2] as f64 / 1000.0;
    let p95 = samples[samples.len() * 95 / 100] as f64 / 1000.0;

    println!("entries_files={}", entries);
    println!("nodes={}", store.len());
    println!("node_size_bytes={}", std::mem::size_of::<Node>());
    println!("name_bytes={}", store.arena().names.len());
    println!("ext_count={}", store.arena().ext_count());
    println!("path_index_capacity={}", store.path_index_capacity());
    println!("path_index_len={}", store.path_index_len());
    println!("id_index_len={}", store.id_index_len());
    println!("bursts_allocated={}", store.bursts_allocated());
    println!("rss_before_kb={}", rss_before);
    println!("rss_after_kb={}", rss_after);
    println!("rss_delta_mb={:.1}", (rss_after as i64 - rss_before as i64) as f64 / 1024.0);
    println!("rss_hwm_after_kb={}", hwm_after.max(hwm_before));
    println!("lookup_p50_us={:.3}", p50);
    println!("lookup_p95_us={:.3}", p95);
}
