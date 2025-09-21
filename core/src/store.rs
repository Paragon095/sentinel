use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Filesystem-backed key/value store used as the default KV engine.
#[derive(Clone)]
pub struct FsKv {
    root: PathBuf,
}

/// Minimal key/value interface over byte keys and values.
pub trait Kv: Clone + Send + Sync + 'static {
    /// Get value bytes for `key`, if present.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    /// Set value bytes for `key`, overwriting if it exists.
    fn put(&self, key: &[u8], val: &[u8]);
    /// Delete `key`; returns `true` if a value existed.
    fn delete(&self, key: &[u8]) -> bool;
}

/// Serde helpers layered on top of any [`Kv`] implementation.
pub trait KvSerde: Kv {
    /// Deserialize type `T` stored at `key` using `bincode`.
    fn get_t<T: DeserializeOwned>(&self, key: &[u8]) -> Result<Option<T>> {
        match self.get(key) {
            Some(bytes) => {
                let v = bincode::deserialize::<T>(&bytes)
                    .with_context(|| "bincode deserialize")?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }
    /// Serialize `val` with `bincode` and store at `key`.
    fn put_t<T: Serialize>(&self, key: &[u8], val: &T) -> Result<()> {
        let buf = bincode::serialize(val).with_context(|| "bincode serialize")?;
        self.put(key, &buf);
        Ok(())
    }
}
impl<T: Kv> KvSerde for T {}

/// Default KV type exported by this crate (FS-backed).
pub type DefaultKv = FsKv;

/// Open an FS-backed KV rooted at `dir` (created if missing).
pub fn open_default<P: AsRef<Path>>(dir: P) -> Result<DefaultKv> {
    let root = dir.as_ref().to_path_buf();
    fs::create_dir_all(&root)
        .with_context(|| format!("create kv dir {}", root.display()))?;
    Ok(FsKv { root })
}

/// Build a namespaced key as bytes: `"{ns}:{key}"`.
pub fn ns(ns: &str, key: &str) -> Vec<u8> {
    let mut s = String::with_capacity(ns.len() + 1 + key.len());
    s.push_str(ns);
    s.push(':');
    s.push_str(key);
    s.into_bytes()
}

/* --------------------- impl FsKv --------------------- */

impl FsKv {
    fn path_for(&self, key: &[u8]) -> PathBuf {
        // Windows-safe: map arbitrary bytes to a hex file name.
        let mut name = String::with_capacity(key.len() * 2);
        for &b in key {
            let hi = (b >> 4) & 0xF;
            let lo = b & 0xF;
            name.push(hex_digit(hi));
            name.push(hex_digit(lo));
        }
        self.root.join(name)
    }
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '?',
    }
}

impl Kv for FsKv {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let path = self.path_for(key);
        let mut f = fs::File::open(&path).ok()?;
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_ok() { Some(buf) } else { None }
    }

    fn put(&self, key: &[u8], val: &[u8]) {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Best-effort atomic-ish write: write temp then rename.
        let tmp = path.with_extension("tmp");
        if let Ok(mut f) = fs::File::create(&tmp) {
            let _ = f.write_all(val);
            let _ = f.sync_all();
            let _ = fs::rename(tmp, path);
        }
    }

    fn delete(&self, key: &[u8]) -> bool {
        let path = self.path_for(key);
        fs::remove_file(path).is_ok()
    }
}
