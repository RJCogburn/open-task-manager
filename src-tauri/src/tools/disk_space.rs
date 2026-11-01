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

struct ScanResult {
    root: PathBuf,
    nodes: Vec<Node>,
}

struct Node {
    name: Box<str>,
    size: u64,
    items: u64,
    is_dir: bool,
    parent: u32,
    children: Vec<u32>,
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

fn flatten(tree: Tree, parent: u32, nodes: &mut Vec<Node>) -> u32 {
    let id = nodes.len() as u32;
    nodes.push(Node {
        name: tree.name,
        size: tree.size,
        items: tree.items,
        is_dir: tree.is_dir,
        parent,
        children: Vec::new(),
    });
    let mut children = tree.children;
    children.sort_unstable_by_key(|c| std::cmp::Reverse(c.size));
    let ids: Vec<u32> = children.into_iter().map(|c| flatten(c, id, nodes)).collect();
    nodes[id as usize].children = ids;
    id
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
        let tree = walk.dir(&root, name);
        let cancelled = walk.cancel.load(Ordering::Relaxed);
        if !cancelled {
            let mut nodes = Vec::new();
            flatten(tree, u32::MAX, &mut nodes);
            *scan.result.lock().unwrap() = Some(ScanResult { root, nodes });
        }
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
    let node = result.nodes.get(id as usize).filter(|n| n.is_dir).ok_or("That folder is not in the scan.")?;
    let entry = |id: u32| {
        let n = &result.nodes[id as usize];
        Entry { id, name: n.name.to_string(), size: n.size, items: n.items, is_dir: n.is_dir }
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
        items: node.items,
        trail,
        children: node.children.iter().take(MAX_CHILDREN).map(|&c| entry(c)).collect(),
        more: node.children.len().saturating_sub(MAX_CHILDREN),
    })
}

fn node_path(result: &ScanResult, id: u32) -> PathBuf {
    let mut parts = Vec::new();
    let mut at = id;
    while at != 0 && at != u32::MAX {
        parts.push(&*result.nodes[at as usize].name);
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
    for (id, node) in result.nodes.iter().enumerate().filter(|(_, n)| !n.is_dir) {
        heap.push(std::cmp::Reverse((node.size, id as u32)));
        if heap.len() > 50 {
            heap.pop();
        }
        let ext = match node.name.rsplit_once('.') {
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
