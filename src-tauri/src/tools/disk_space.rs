//! Disk space analysis in the spirit of ncdu and Filelight: walk a folder on
//! one filesystem, add up the space each file really uses on disk, then let
//! the window browse the result folder by folder.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::AppState;

#[derive(Default)]
pub struct ScanState {
    running: AtomicBool,
    cancel: Arc<AtomicBool>,
    result: Mutex<Option<ScanResult>>,
}

/// A finished scan, kept compact: a big drive has millions of entries. Names
/// share one string, and each folder's contents sit side by side in `nodes`,
/// largest first, so an entry needs 32 bytes plus its name.
struct ScanResult {
    root: PathBuf,
    nodes: Vec<Node>,
    names: String,
}

struct Node {
    size: u64,
    /// Where the name starts in `names`, and its length.
    name: u32,
    name_len: u32,
    items: u32,
    parent: u32,
    /// The contents: `children` entries starting at `first_child`.
    first_child: u32,
    children: u32,
}

impl Node {
    fn is_dir(&self) -> bool {
        self.first_child != NO_CHILDREN
    }

    fn child_ids(&self) -> std::ops::Range<u32> {
        if self.is_dir() { self.first_child..self.first_child + self.children } else { 0..0 }
    }
}

/// `first_child` of a file, telling it from an empty folder.
const NO_CHILDREN: u32 = u32::MAX;

impl ScanResult {
    fn name(&self, id: u32) -> &str {
        let n = &self.nodes[id as usize];
        &self.names[n.name as usize..(n.name + n.name_len) as usize]
    }
}

/// Hands memory the allocator holds but no longer uses back to the system.
/// After a scan on many threads that can be hundreds of megabytes.
fn release_memory() {
    // SAFETY: malloc_trim only returns free pages; it touches nothing in use.
    unsafe { libc::malloc_trim(0) };
}

/// The tree as the walk builds it, before it is flattened for browsing.
struct Tree {
    name: Box<str>,
    size: u64,
    items: u64,
    is_dir: bool,
    children: Vec<Tree>,
}

struct Walk {
    device: u64,
    cancel: Arc<AtomicBool>,
    files: AtomicU64,
    bytes: AtomicU64,
    errors: AtomicU64,
    /// Files with several hard links are counted once, like ncdu does.
    seen_links: Mutex<HashSet<(u64, u64)>>,
}

impl Walk {
    fn dir(&self, path: &Path, name: Box<str>) -> Tree {
        let mut tree = Tree { name, size: 0, items: 1, is_dir: true, children: Vec::new() };
        if self.cancel.load(Ordering::Relaxed) {
            return tree;
        }
        let Ok(entries) = fs::read_dir(path) else {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return tree;
        };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            // DirEntry::metadata does not follow symlinks.
            let Ok(md) = entry.metadata() else {
                self.errors.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            let name: Box<str> = entry.file_name().to_string_lossy().into();
            if md.is_dir() {
                // Stay on this filesystem, so / doesn't wander into /proc or other drives.
                if md.dev() == self.device {
                    subdirs.push((entry.path(), name));
                }
                continue;
            }
            let mut size = md.blocks() * 512;
            if md.nlink() > 1 && !self.seen_links.lock().unwrap().insert((md.dev(), md.ino())) {
                size = 0;
            }
            self.files.fetch_add(1, Ordering::Relaxed);
            self.bytes.fetch_add(size, Ordering::Relaxed);
            tree.size += size;
            tree.items += 1;
            tree.children.push(Tree { name, size, items: 1, is_dir: false, children: Vec::new() });
        }
        let subtrees: Vec<Tree> = subdirs.into_par_iter().map(|(path, name)| self.dir(&path, name)).collect();
        for sub in subtrees {
            tree.size += sub.size;
            tree.items += sub.items;
            tree.children.push(sub);
        }
        tree
    }
}

fn node(tree: &Tree, parent: u32, names: &mut String) -> Node {
    let name = names.len() as u32;
    names.push_str(&tree.name);
    Node {
        size: tree.size,
        name,
        name_len: tree.name.len() as u32,
        items: tree.items.min(u32::MAX as u64) as u32,
        parent,
        first_child: if tree.is_dir { 0 } else { NO_CHILDREN },
        children: 0,
    }
}

/// Turns the walked tree into the compact result: the root is entry 0, and
/// each folder's contents are placed together, largest first.
fn flatten(tree: Tree, root: PathBuf) -> ScanResult {
    let mut result = ScanResult { root, nodes: Vec::new(), names: String::new() };
    result.nodes.push(node(&tree, u32::MAX, &mut result.names));
    place_children(tree, 0, &mut result);
    result.nodes.shrink_to_fit();
    result.names.shrink_to_fit();
    result
}

fn place_children(tree: Tree, id: u32, result: &mut ScanResult) {
    let mut children = tree.children;
    if !tree.is_dir {
        return;
    }
    children.sort_unstable_by_key(|c| std::cmp::Reverse(c.size));
    let first = result.nodes.len() as u32;
    for child in &children {
        let n = node(child, id, &mut result.names);
        result.nodes.push(n);
    }
    let parent = &mut result.nodes[id as usize];
    parent.first_child = first;
    parent.children = children.len() as u32;
    for (i, child) in children.into_iter().enumerate() {
        place_children(child, first + i as u32, result);
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    files: u64,
    bytes: u64,
    errors: u64,
    seconds: f64,
    done: bool,
    cancelled: bool,
    error: Option<String>,
}

#[tauri::command]
pub fn scan_start(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let root = PathBuf::from(&path);
    let md = fs::metadata(&root).map_err(|e| format!("Can't open {path}: {e}"))?;
    if !md.is_dir() {
        return Err(format!("{path} is not a folder."));
    }
    let scan = state.scan.clone();
    if scan.running.swap(true, Ordering::SeqCst) {
        return Err("A scan is already running.".into());
    }
    scan.cancel.store(false, Ordering::SeqCst);
    // Let the last result go before the new one is built, rather than holding both.
    *scan.result.lock().unwrap() = None;
    release_memory();
    let walk = Arc::new(Walk {
        device: md.dev(),
        cancel: scan.cancel.clone(),
        files: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        errors: AtomicU64::new(0),
        seen_links: Mutex::new(HashSet::new()),
    });
    let started = Instant::now();
    let finished = Arc::new(AtomicBool::new(false));

    let (w, f, a) = (walk.clone(), finished.clone(), app.clone());
    std::thread::spawn(move || {
        while !f.load(Ordering::Relaxed) {
            let _ = a.emit("scan-progress", progress(&w, started, false, false, None));
            std::thread::sleep(Duration::from_millis(250));
        }
    });

    std::thread::spawn(move || {
        let name: Box<str> = root.to_string_lossy().into();
        // Threads for this scan only; they end with it rather than idling for good.
        let tree = match rayon::ThreadPoolBuilder::new().thread_name(|i| format!("disk-scan-{i}")).build() {
            Ok(pool) => pool.install(|| walk.dir(&root, name)),
            Err(_) => walk.dir(&root, name),
        };
        let cancelled = walk.cancel.load(Ordering::Relaxed);
        if cancelled {
            drop(tree);
        } else {
            *scan.result.lock().unwrap() = Some(flatten(tree, root));
        }
        // The walk's tree is gone now; give its memory back.
        walk.seen_links.lock().unwrap().clear();
        release_memory();
        finished.store(true, Ordering::Relaxed);
        scan.running.store(false, Ordering::SeqCst);
        let _ = app.emit("scan-progress", progress(&walk, started, true, cancelled, None));
    });
    Ok(())
}

fn progress(w: &Walk, started: Instant, done: bool, cancelled: bool, error: Option<String>) -> Progress {
    Progress {
        files: w.files.load(Ordering::Relaxed),
        bytes: w.bytes.load(Ordering::Relaxed),
        errors: w.errors.load(Ordering::Relaxed),
        seconds: started.elapsed().as_secs_f64(),
        done,
        cancelled,
        error,
    }
}

#[tauri::command]
pub fn scan_cancel(state: State<'_, AppState>) {
    state.scan.cancel.store(true, Ordering::SeqCst);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    id: u32,
    name: String,
    size: u64,
    items: u64,
    is_dir: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    id: u32,
    path: String,
    size: u64,
    items: u64,
    /// From the scanned root down to this folder.
    trail: Vec<Entry>,
    children: Vec<Entry>,
    more: usize,
}

const MAX_CHILDREN: usize = 400;

/// One folder of the last scan; id 0 is the folder that was scanned.
#[tauri::command]
pub fn scan_folder(state: State<'_, AppState>, id: u32) -> Result<Folder, String> {
    let guard = state.scan.result.lock().unwrap();
    let result = guard.as_ref().ok_or("Nothing has been scanned yet.")?;
    let node = result.nodes.get(id as usize).filter(|n| n.is_dir()).ok_or("That folder is not in the scan.")?;
    let entry = |id: u32| {
        let n = &result.nodes[id as usize];
        Entry { id, name: result.name(id).to_string(), size: n.size, items: n.items as u64, is_dir: n.is_dir() }
    };
    let mut trail = Vec::new();
    let mut at = id;
    while at != u32::MAX {
        trail.push(entry(at));
        at = result.nodes[at as usize].parent;
    }
    trail.reverse();
    Ok(Folder {
        id,
        path: node_path(result, id).to_string_lossy().into_owned(),
        size: node.size,
        items: node.items as u64,
        trail,
        children: node.child_ids().take(MAX_CHILDREN).map(entry).collect(),
        more: (node.children as usize).saturating_sub(MAX_CHILDREN),
    })
}

fn node_path(result: &ScanResult, id: u32) -> PathBuf {
    let mut parts = Vec::new();
    let mut at = id;
    while at != 0 && at != u32::MAX {
        parts.push(result.name(at));
        at = result.nodes[at as usize].parent;
    }
    let mut path = result.root.clone();
    path.extend(parts.into_iter().rev());
    path
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BigFile {
    path: String,
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileType {
    extension: String,
    size: u64,
    count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    largest: Vec<BigFile>,
    types: Vec<FileType>,
}

/// The largest files and the space used by each kind of file.
#[tauri::command]
pub fn scan_summary(state: State<'_, AppState>) -> Result<Summary, String> {
    let guard = state.scan.result.lock().unwrap();
    let result = guard.as_ref().ok_or("Nothing has been scanned yet.")?;
    let mut heap = BinaryHeap::new();
    let mut types: HashMap<String, (u64, u64)> = HashMap::new();
    for (id, node) in result.nodes.iter().enumerate().filter(|(_, n)| !n.is_dir()) {
        heap.push(std::cmp::Reverse((node.size, id as u32)));
        if heap.len() > 50 {
            heap.pop();
        }
        let ext = match result.name(id as u32).rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && ext.len() <= 10 => ext.to_ascii_lowercase(),
            _ => "(none)".into(),
        };
        let t = types.entry(ext).or_default();
        t.0 += node.size;
        t.1 += 1;
    }
    let mut largest: Vec<BigFile> = heap
        .into_iter()
        .map(|std::cmp::Reverse((size, id))| BigFile {
            path: node_path(result, id).to_string_lossy().into_owned(),
            size,
        })
        .collect();
    largest.sort_by_key(|f| std::cmp::Reverse(f.size));
    let mut types: Vec<FileType> =
        types.into_iter().map(|(extension, (size, count))| FileType { extension, size, count }).collect();
    types.sort_by_key(|t| std::cmp::Reverse(t.size));
    types.truncate(24);
    Ok(Summary { largest, types })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Tree {
        Tree { name: name.into(), size, items: 1, is_dir: false, children: Vec::new() }
    }

    fn dir(name: &str, children: Vec<Tree>) -> Tree {
        let size = children.iter().map(|c| c.size).sum();
        let items = 1 + children.iter().map(|c| c.items).sum::<u64>();
        Tree { name: name.into(), size, items, is_dir: true, children }
    }

    #[test]
    fn keeps_the_tree_compactly() {
        let tree = dir(
            "/scan",
            vec![
                file("small.txt", 10),
                dir("docs", vec![file("a.pdf", 300), file("b.md", 5)]),
                dir("empty", vec![]),
                file("big.iso", 900),
            ],
        );
        let r = flatten(tree, PathBuf::from("/scan"));
        let root = &r.nodes[0];
        assert!(root.is_dir());
        let names: Vec<&str> = root.child_ids().map(|id| r.name(id)).collect();
        assert_eq!(names, ["big.iso", "docs", "small.txt", "empty"], "largest first");
        let docs = root.child_ids().find(|&id| r.name(id) == "docs").unwrap();
        let inside: Vec<(&str, u64)> =
            r.nodes[docs as usize].child_ids().map(|id| (r.name(id), r.nodes[id as usize].size)).collect();
        assert_eq!(inside, [("a.pdf", 300), ("b.md", 5)]);
        let pdf = r.nodes[docs as usize].first_child;
        assert_eq!(node_path(&r, pdf), PathBuf::from("/scan/docs/a.pdf"));
        let empty = root.child_ids().find(|&id| r.name(id) == "empty").unwrap();
        assert!(r.nodes[empty as usize].is_dir() && r.nodes[empty as usize].child_ids().is_empty());
        assert!(!r.nodes[pdf as usize].is_dir());
        assert_eq!(std::mem::size_of::<Node>(), 32);
    }
}
