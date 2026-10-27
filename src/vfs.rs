use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, IdbDatabase, IdbObjectStore, IdbTransaction, IdbOpenDbRequest};
use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::{Serialize, Deserialize};

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
    // simple in-memory cache; persistence is async via IndexedDB
}

impl Vfs {
    pub fn new() -> Self {
        let mut root = Node::new_dir();
        // seed standard hierarchy
        if let Node::Dir { children, .. } = &mut root {
            children.insert("root".into(), Node::new_dir());
            children.insert("bin".into(),  Node::new_dir());
            children.insert("tmp".into(),  Node::new_dir());
            children.insert("etc".into(),  Node::new_dir());
            children.insert("home".into(), Node::new_dir());
            // /etc/passwd stub
            if let Node::Dir { children: etc, .. } = children.get_mut("etc").unwrap() {
                etc.insert("passwd".into(), Node::new_file(
                    b"root:x:0:0:root:/root:/bin/sh\n".to_vec()
                ));
            }
        }
        Self { root }
    }

    pub async fn init_persistence(&mut self) -> Result<(), VfsError> {
        // Load from IndexedDB if present
        if let Some(db) = open_idb().await? {
            if let Ok(Some(data)) = load_snapshot(&db).await {
                if let Ok(node) = serde_json::from_slice::<Node>(&data) {
                    self.root = node;
                }
            }
        }
        Ok(())
    }

    pub async fn persist(&self) -> Result<(), VfsError> {
        if let Some(db) = open_idb().await? {
            let data = serde_json::to_vec(&self.root)
                .map_err(|e| VfsError::Io(e.to_string()))?;
            save_snapshot(&db, &data).await?;
        }
        Ok(())
    }

    fn resolve<'a>(&'a self, path: &str) -> Result<&'a Node, VfsError> {
        let p = normalize(path);
        let mut cur = &self.root;
        for comp in p.components().skip(1) { // skip root
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

        // ensure parent exists
        self.mkdir_p(&parent.to_string_lossy()).await?;

        let parent_node = self.resolve_mut(&parent.to_string_lossy())?;
        if let Node::Dir { children, .. } = parent_node {
            children.insert(name, Node::new_file(data.to_vec()));
        }
        self.persist().await?;
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
        self.persist().await?;
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
        self.persist().await?;
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
            std::path::Component::RootDir => {},
            std::path::Component::Normal(s) => p.push(s),
            std::path::Component::ParentDir => { p.pop(); },
            _ => {}
        }
    }
    p
}

// ---------- IndexedDB helpers ----------

async fn open_idb() -> Result<Option<IdbDatabase>, VfsError> {
    let window = window().ok_or(VfsError::Db("no window".into()))?;
    let factory = window.indexed_db()
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?
        .ok_or(VfsError::Db("no indexedDB".into()))?;

    let req: IdbOpenDbRequest = factory.open_with_u32("mobile-linux-vfs", 1)
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;

    // onupgradeneeded
    let onupgradeneeded = Closure::wrap(Box::new(move |e: web_sys::Event| {
        let target = e.target().unwrap();
        let req: IdbOpenDbRequest = target.dyn_into().unwrap();
        let db = req.result().unwrap().dyn_into::<IdbDatabase>().unwrap();
        if !db.object_store_names().contains("snapshot") {
            db.create_object_store("snapshot").unwrap();
        }
    }) as Box<dyn FnMut(_)>);
    req.set_onupgradeneeded(Some(onupgradeneeded.as_ref().unchecked_ref()));
    onupgradeneeded.forget();

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let resolve = resolve.clone();
        let reject = reject.clone();
        let onsuccess = Closure::wrap(Box::new(move |e: web_sys::Event| {
            let target = e.target().unwrap();
            let req: IdbOpenDbRequest = target.dyn_into().unwrap();
            resolve.call1(&JsValue::NULL, &req.result().unwrap()).unwrap();
        }) as Box<dyn FnMut(_)>);
        req.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        onsuccess.forget();

        let onerror = Closure::wrap(Box::new(move |e: web_sys::Event| {
            reject.call1(&JsValue::NULL, &e).unwrap();
        }) as Box<dyn FnMut(_)>);
        req.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    });

    let db = JsFuture::from(promise).await
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?
        .dyn_into::<IdbDatabase>()
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    Ok(Some(db))
}

async fn load_snapshot(db: &IdbDatabase) -> Result<Option<Vec<u8>>, VfsError> {
    let tx = db.transaction_with_str_and_mode("snapshot", web_sys::IdbTransactionMode::Readonly)
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    let store = tx.object_store("snapshot")
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    let req = store.get(&JsValue::from_str("root"))
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let onsuccess = Closure::wrap(Box::new(move |e: web_sys::Event| {
            let target = e.target().unwrap();
            let req: web_sys::IdbRequest = target.dyn_into().unwrap();
            resolve.call1(&JsValue::NULL, &req.result().unwrap()).unwrap();
        }) as Box<dyn FnMut(_)>);
        req.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        onsuccess.forget();
        let onerror = Closure::wrap(Box::new(move |e: web_sys::Event| {
            reject.call1(&JsValue::NULL, &e).unwrap();
        }) as Box<dyn FnMut(_)>);
        req.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    });

    let val = JsFuture::from(promise).await
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    if val.is_null() || val.is_undefined() {
        return Ok(None);
    }
    let arr = Uint8Array::new(&val);
    let mut buf = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut buf);
    Ok(Some(buf))
}

async fn save_snapshot(db: &IdbDatabase, data: &[u8]) -> Result<(), VfsError> {
    let tx = db.transaction_with_str_and_mode("snapshot", web_sys::IdbTransactionMode::Readwrite)
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    let store = tx.object_store("snapshot")
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    let arr = Uint8Array::new_with_length(data.len() as u32);
    arr.copy_from(data);
    store.put_with_key(&arr, &JsValue::from_str("root"))
        .map_err(|e| VfsError::Db(format!("{:?}", e)))?;
    Ok(())
}
