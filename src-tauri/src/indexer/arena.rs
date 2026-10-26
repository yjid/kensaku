use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub type NodeId = u32;

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub parent: NodeId,
    pub name_off: u32,
    pub name_len: u32,
    pub ext_id: u16,
    pub flags: u8,
    pub size: u64,
    pub mtime: u64,
    pub dev: u64,
    pub ino: u64,
    pub char_bloom: u64,
    pub node_id: u64,
}

impl Node {
    pub const FLAG_DIR: u8 = 1;
    pub const FLAG_CONTENT_PENDING: u8 = 2;
    pub fn is_dir(&self) -> bool {
        self.flags & Self::FLAG_DIR != 0
    }
    pub fn content_pending(&self) -> bool {
        self.flags & Self::FLAG_CONTENT_PENDING != 0
    }
    pub fn set_dir(&mut self, v: bool) {
        if v {
            self.flags |= Self::FLAG_DIR;
        } else {
            self.flags &= !Self::FLAG_DIR;
        }
    }
    pub fn set_content_pending(&mut self, v: bool) {
        if v {
            self.flags |= Self::FLAG_CONTENT_PENDING;
        } else {
            self.flags &= !Self::FLAG_CONTENT_PENDING;
        }
    }
}

pub const NO_PARENT: NodeId = u32::MAX;

#[derive(Debug, Default)]
pub struct NameArena {
    buf: Vec<u8>,
}

impl NameArena {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
    pub fn push(&mut self, name: &str) -> (u32, u32) {
        let off = self.buf.len() as u32;
        self.buf.extend_from_slice(name.as_bytes());
        (off, name.len() as u32)
    }
    pub fn get(&self, off: u32, len: u32) -> &str {
        let start = off as usize;
        let end = start + len as usize;
        if end > self.buf.len() {
            return "";
        }
        std::str::from_utf8(&self.buf[start..end]).unwrap_or("")
    }
}

#[derive(Debug)]
pub struct NodeArena {
    pub nodes: Vec<Node>,
    pub names: NameArena,
    exts: Vec<Box<str>>,
    ext_index: HashMap<Box<str>, u16>,
}

impl Default for NodeArena {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeArena {
    pub fn new() -> Self {
        // Reserve ext id 0 for "no extension".
        let exts = vec![Box::from("")];
        Self {
            nodes: Vec::new(),
            names: NameArena::new(),
            exts,
            ext_index: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, idx: u32) -> &Node {
        &self.nodes[idx as usize]
    }

    pub fn node_mut(&mut self, idx: u32) -> &mut Node {
        &mut self.nodes[idx as usize]
    }

    pub fn name(&self, idx: u32) -> &str {
        let n = &self.nodes[idx as usize];
        self.names.get(n.name_off, n.name_len)
    }

    pub fn extension(&self, idx: u32) -> Option<String> {
        let n = &self.nodes[idx as usize];
        if n.ext_id == 0 {
            None
        } else {
            Some(self.exts[n.ext_id as usize].to_string())
        }
    }

    pub fn ext_str(&self, ext_id: u16) -> &str {
        if ext_id == 0 {
            ""
        } else {
            &self.exts[ext_id as usize]
        }
    }

    pub fn intern_ext(&mut self, ext: Option<&str>) -> u16 {
        match ext {
            None => 0,
            Some("") => 0,
            Some(e) => {
                let key: Box<str> = Box::from(e);
                if let Some(&id) = self.ext_index.get(&key) {
                    return id;
                }
                let id = self.exts.len() as u16;
                self.exts.push(key.clone());
                self.ext_index.insert(key, id);
                id
            }
        }
    }

    pub fn push(&mut self, node: Node) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(node);
        idx
    }

    pub fn path(&self, idx: u32) -> PathBuf {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = Some(idx);
        let mut guard = 0;
        while let Some(i) = cur {
            if guard > self.nodes.len() + 1 {
                break;
            }
            guard += 1;
            let node = &self.nodes[i as usize];
            parts.push(self.names.get(node.name_off, node.name_len));
            if node.parent == NO_PARENT {
                break;
            } else {
                cur = Some(node.parent);
            }
        }
        parts.reverse();
        let mut p = PathBuf::new();
        for part in parts {
            if !part.is_empty() {
                p = p.join(part);
            }
        }
        p
    }

    /// Reconstruct parent index if needed (already stored).
    pub fn parent(&self, idx: u32) -> NodeId {
        self.nodes[idx as usize].parent
    }
}

pub fn path_hash(path: &Path) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in path.to_string_lossy().as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
