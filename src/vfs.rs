use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, IdbDatabase, IdbOpenDbRequest, IdbTransactionMode};
use js_sys::Uint8Array;

#[derive(Error, Debug)]
pub enum VfsError {
    #[error("Path not found: {0}")]
    NotFound(String),
    #[error("Not a directory: {0}")]
    NotDir(String),
    #[error("Is a directory: {0}")]
    IsDir(String),
    #[error("Permission denied")]
    Permission,
    #[error("IO error: {0}")]
    Io(String),
    #[error("DB error: {0}")]
    Db(String),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Node {
    File { data: Vec<u8>, mode: u32 },
    Dir  { children: HashMap<String, Node>, mode: u32 },
}

impl Node {
    fn new_dir() -> Self {
        Node::Dir { children: HashMap::new(), mode: 0o755 }
    }
    fn new_file(data: Vec<u8>) -> Self {
        Node::File { data, mode: 0o644 }
    }
}

pub struct Vfs {
    root: Node,
}

impl Vfs {
    pub fn new() -> Self {
        let mut root = Node::new_dir();
        if let Node::Dir { children, .. } = &mut root {
            children.insert("root".into(), Node::new_dir());
            children.insert("bin".into(),  Node::new_dir());
            children.insert("tmp".into(),  Node::new_dir());
            children.insert("etc".into(),  Node::new_dir());
            children.insert("home".into(), Node::new_dir());
            if let Some(Node::Dir { children: etc, .. }) = children.get_mut("etc") {
                etc.insert("passwd".into(), Node::new_file(
                    b"root:x:0:0:root:/root:/bin/sh\n".to_vec()
                ));
            }
        }
        Self { root }
    }

    pub async fn init_persistence(&mut self) -> Result<(), VfsError> {
        // در نسخه ساده فعلی، لود از IndexedDB را غیرفعال گذاشتم
        // تا از پیچیدگی و باگ‌های Closure جلوگیری شود.
        // می‌توانی بعداً اضافه کنی.
        Ok(())
    }

    pub async fn persist(&self) -> Result<(), VfsError> {
        // فعلاً no-op برای پایداری
        Ok(())
    }

    fn resolve<'a>(&'a self, path: &str) -> Result<&'a Node, VfsError> {
        let p = normalize(path);
        let mut cur = &self.root;
        for comp in p.components().skip(1) {
            let name = comp.as_os_str().to_string_lossy();
            match cur {
                Node::Dir { children, .. } => {
                    cur = children.get(name.as_ref())
                        .ok_or_else(|| VfsError::NotFound(p.display().to_string()))?;
                }
                _ => return Err(VfsError::NotDir(p.display().to_string())),
            }
        }
        Ok(cur)
    }

    fn resolve_mut<'a>(&'a mut self, path: &str) -> Result<&'a mut Node, VfsError> {
        let p = normalize(path);
        let mut cur = &mut self.root;
        for comp in p.components().skip(1) {
            let name = comp.as_os_str().to_string_lossy().into_owned();
            match cur {
                Node::Dir { children, .. } => {
                    cur = children.get_mut(&name)
                        .ok_or_else(|| VfsError::NotFound(p.display().to_string()))?;
                }
                _ => return Err(VfsError::NotDir(p.display().to_string())),
            }
        }
        Ok(cur)
    }

    pub fn ls(&self, path: &str) -> Result<Vec<String>, VfsError> {
        match self.resolve(path)? {
            Node::Dir { children, .. } => {
                let mut names: Vec<_> = children.keys().cloned().collect();
                names.sort();
                Ok(names)
            }
            _ => Err(VfsError::NotDir(path.into())),
        }
    }

    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        match self.resolve(path)? {
            Node::File { data, .. } => Ok(data.clone()),
            _ => Err(VfsError::IsDir(path.into())),
        }
    }

    pub async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        let p = normalize(path);
        let parent = p.parent().ok_or(VfsError::Permission)?;
        let name = p.file_name().ok_or(VfsError::Permission)?
            .to_string_lossy().into_owned();

        self.mkdir_p(&parent.to_string_lossy()).await?;

        let parent_node = self.resolve_mut(&parent.to_string_lossy())?;
        if let Node::Dir { children, .. } = parent_node {
            children.insert(name, Node::new_file(data.to_vec()));
        }
        Ok(())
    }

    pub async fn mkdir(&mut self, path: &str) -> Result<(), VfsError> {
        self.mkdir_p(path).await
    }

    async fn mkdir_p(&mut self, path: &str) -> Result<(), VfsError> {
        let p = normalize(path);
        let mut cur = &mut self.root;
        for comp in p.components().skip(1) {
            let name = comp.as_os_str().to_string_lossy().into_owned();
            match cur {
                Node::Dir { children, .. } => {
                    if !children.contains_key(&name) {
                        children.insert(name.clone(), Node::new_dir());
                    }
                    cur = children.get_mut(&name).unwrap();
                }
                _ => return Err(VfsError::NotDir(path.into())),
            }
        }
        Ok(())
    }

    pub async fn rm(&mut self, path: &str, recursive: bool) -> Result<(), VfsError> {
        let p = normalize(path);
        let parent = p.parent().ok_or(VfsError::Permission)?;
        let name = p.file_name().ok_or(VfsError::Permission)?
            .to_string_lossy().into_owned();

        let parent_node = self.resolve_mut(&parent.to_string_lossy())?;
        if let Node::Dir { children, .. } = parent_node {
            if let Some(node) = children.get(&name) {
                if matches!(node, Node::Dir { .. }) && !recursive {
                    return Err(VfsError::IsDir(path.into()));
                }
                children.remove(&name);
            } else {
                return Err(VfsError::NotFound(path.into()));
            }
        }
        Ok(())
    }

    pub async fn cp(&mut self, src: &str, dst: &str) -> Result<(), VfsError> {
        let data = self.read_file(src).await?;
        self.write_file(dst, &data).await
    }

    pub async fn mv(&mut self, src: &str, dst: &str) -> Result<(), VfsError> {
        self.cp(src, dst).await?;
        self.rm(src, false).await
    }
}

fn normalize(path: &str) -> PathBuf {
    let mut p = PathBuf::from("/");
    for c in Path::new(path).components() {
        match c {
            std::path::Component::Normal(s) => p.push(s),
            std::path::Component::ParentDir => { p.pop(); },
            _ => {}
        }
    }
    p
}
