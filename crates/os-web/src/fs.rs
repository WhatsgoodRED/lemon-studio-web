// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! The file system the app's web-relevant paths use (samples, recordings, exports, projects): `std::fs` on native,
//! an in-memory one on the web (wasm32), with the page's way in and out (`lemon_fs_*`).

use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::*;

/// Whether `p` is there (`Path::exists`, which on the web asks the real file system: there is none).
pub fn exists(p: impl AsRef<Path>) -> bool {
    metadata(p).is_ok()
}

pub fn is_file(p: impl AsRef<Path>) -> bool {
    metadata(p).is_ok_and(|m| m.is_file())
}

pub fn is_dir(p: impl AsRef<Path>) -> bool {
    metadata(p).is_ok_and(|m| m.is_dir())
}

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::path::{Component, Path, PathBuf};
    use std::rc::Rc;
    use std::time::SystemTime;

    use wasm_bindgen::prelude::*;

    struct Node {
        data: Rc<RefCell<Vec<u8>>>,
        modified: SystemTime,
    }

    #[derive(Default)]
    struct Fs {
        files: BTreeMap<PathBuf, Node>,
        dirs: BTreeSet<PathBuf>,
        /// Counts writes, so the page can tell what changed (`lemon_fs_version`).
        version: u64,
    }

    thread_local! {
        static FS: RefCell<Fs> = RefCell::new(Fs::default());
    }

    /// Seconds since 1970 for a file's modification time: the page's clock (std's `SystemTime::now` panics here).
    fn now() -> SystemTime {
        let ms = js_sys_now();
        SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64)
    }

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = Date, js_name = now)]
        fn js_sys_now() -> f64;
    }

    /// `p` absolute and without `.` and `..`.
    pub fn norm(p: &Path) -> PathBuf {
        let mut out = PathBuf::from("/");
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::Normal(n) => out.push(n),
                Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            }
        }
        out
    }

    fn not_found(p: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, format!("{}: not found", p.display()))
    }

    fn add_parents(fs: &mut Fs, p: &Path) {
        let mut d = p.parent();
        while let Some(x) = d {
            fs.dirs.insert(x.to_path_buf());
            d = x.parent();
        }
    }

    fn put(p: &Path, data: Vec<u8>) {
        let p = norm(p);
        FS.with(|fs| {
            let mut fs = fs.borrow_mut();
            add_parents(&mut fs, &p);
            fs.version += 1;
            fs.files.insert(p, Node { data: Rc::new(RefCell::new(data)), modified: now() });
        });
    }

    fn get(p: &Path) -> Option<Rc<RefCell<Vec<u8>>>> {
        let p = norm(p);
        FS.with(|fs| fs.borrow().files.get(&p).map(|n| n.data.clone()))
    }

    pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        let p = p.as_ref();
        get(p).map(|d| d.borrow().clone()).ok_or_else(|| not_found(p))
    }

    pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
        String::from_utf8(read(p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn write(p: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
        put(p.as_ref(), contents.as_ref().to_vec());
        Ok(())
    }

    pub fn create_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
        let p = norm(p.as_ref());
        FS.with(|fs| {
            let mut fs = fs.borrow_mut();
            add_parents(&mut fs, &p);
            fs.dirs.insert(p);
        });
        Ok(())
    }

    pub fn create_dir(p: impl AsRef<Path>) -> io::Result<()> {
        create_dir_all(p)
    }

    pub fn remove_file(p: impl AsRef<Path>) -> io::Result<()> {
        let p = norm(p.as_ref());
        FS.with(|fs| {
            let mut fs = fs.borrow_mut();
            fs.version += 1;
            fs.files.remove(&p).map(|_| ()).ok_or_else(|| not_found(&p))
        })
    }

    pub fn remove_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
        let p = norm(p.as_ref());
        FS.with(|fs| {
            let mut fs = fs.borrow_mut();
            fs.version += 1;
            fs.files.retain(|k, _| !k.starts_with(&p));
            fs.dirs.retain(|k| !k.starts_with(&p));
        });
        Ok(())
    }

    pub fn remove_dir(p: impl AsRef<Path>) -> io::Result<()> {
        remove_dir_all(p)
    }

    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        let (from, to) = (norm(from.as_ref()), norm(to.as_ref()));
        FS.with(|fs| {
            let mut fs = fs.borrow_mut();
            let node = fs.files.remove(&from).ok_or_else(|| not_found(&from))?;
            add_parents(&mut fs, &to);
            fs.version += 1;
            fs.files.insert(to, node);
            Ok(())
        })
    }

    pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
        let data = read(from)?;
        let n = data.len() as u64;
        write(to, data)?;
        Ok(n)
    }

    pub fn canonicalize(p: impl AsRef<Path>) -> io::Result<PathBuf> {
        let p = norm(p.as_ref());
        metadata(&p).map(|_| p)
    }

    #[derive(Clone, Debug)]
    pub struct Metadata {
        len: u64,
        dir: bool,
        modified: SystemTime,
    }

    impl Metadata {
        pub fn len(&self) -> u64 {
            self.len
        }
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
        pub fn is_file(&self) -> bool {
            !self.dir
        }
        pub fn is_dir(&self) -> bool {
            self.dir
        }
        pub fn is_symlink(&self) -> bool {
            false
        }
        pub fn modified(&self) -> io::Result<SystemTime> {
            Ok(self.modified)
        }
        pub fn file_type(&self) -> FileType {
            FileType { dir: self.dir }
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub struct FileType {
        dir: bool,
    }

    impl FileType {
        pub fn is_dir(&self) -> bool {
            self.dir
        }
        pub fn is_file(&self) -> bool {
            !self.dir
        }
        pub fn is_symlink(&self) -> bool {
            false
        }
    }

    pub fn metadata(p: impl AsRef<Path>) -> io::Result<Metadata> {
        let p = norm(p.as_ref());
        FS.with(|fs| {
            let fs = fs.borrow();
            if let Some(n) = fs.files.get(&p) {
                return Ok(Metadata { len: n.data.borrow().len() as u64, dir: false, modified: n.modified });
            }
            if p == Path::new("/") || fs.dirs.contains(&p) {
                return Ok(Metadata { len: 0, dir: true, modified: SystemTime::UNIX_EPOCH });
            }
            Err(not_found(&p))
        })
    }

    pub fn symlink_metadata(p: impl AsRef<Path>) -> io::Result<Metadata> {
        metadata(p)
    }

    pub struct DirEntry {
        path: PathBuf,
    }

    impl DirEntry {
        pub fn path(&self) -> PathBuf {
            self.path.clone()
        }
        pub fn file_name(&self) -> OsString {
            self.path.file_name().map(|n| n.to_os_string()).unwrap_or_default()
        }
        pub fn metadata(&self) -> io::Result<Metadata> {
            metadata(&self.path)
        }
        pub fn file_type(&self) -> io::Result<FileType> {
            metadata(&self.path).map(|m| m.file_type())
        }
    }

    pub struct ReadDir(std::vec::IntoIter<PathBuf>);

    impl Iterator for ReadDir {
        type Item = io::Result<DirEntry>;
        fn next(&mut self) -> Option<Self::Item> {
            self.0.next().map(|path| Ok(DirEntry { path }))
        }
    }

    pub fn read_dir(p: impl AsRef<Path>) -> io::Result<ReadDir> {
        let p = norm(p.as_ref());
        if !metadata(&p)?.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotADirectory, format!("{}: not a folder", p.display())));
        }
        let mut v: Vec<PathBuf> = FS.with(|fs| {
            let fs = fs.borrow();
            let files = fs.files.keys().filter(|k| k.parent() == Some(&p)).cloned();
            let dirs = fs.dirs.iter().filter(|k| k.parent() == Some(&p)).cloned();
            files.chain(dirs).collect()
        });
        v.sort();
        v.dedup();
        Ok(ReadDir(v.into_iter()))
    }

    /// An open file: its bytes are shared with the file system, so what is written shows at once.
    pub struct File {
        data: Rc<RefCell<Vec<u8>>>,
        pos: u64,
        append: bool,
    }

    impl File {
        pub fn open(p: impl AsRef<Path>) -> io::Result<File> {
            let p = p.as_ref();
            let data = get(p).ok_or_else(|| not_found(p))?;
            Ok(File { data, pos: 0, append: false })
        }
        pub fn create(p: impl AsRef<Path>) -> io::Result<File> {
            put(p.as_ref(), Vec::new());
            File::open(p)
        }
        pub fn options() -> OpenOptions {
            OpenOptions::new()
        }
        pub fn metadata(&self) -> io::Result<Metadata> {
            Ok(Metadata { len: self.data.borrow().len() as u64, dir: false, modified: now() })
        }
        pub fn set_len(&self, n: u64) -> io::Result<()> {
            self.data.borrow_mut().resize(n as usize, 0);
            Ok(())
        }
        pub fn sync_all(&self) -> io::Result<()> {
            Ok(())
        }
        pub fn sync_data(&self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Read for File {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let d = self.data.borrow();
            let at = (self.pos as usize).min(d.len());
            let n = buf.len().min(d.len() - at);
            buf[..n].copy_from_slice(&d[at..at + n]);
            self.pos += n as u64;
            Ok(n)
        }
    }

    impl Write for File {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut d = self.data.borrow_mut();
            if self.append {
                self.pos = d.len() as u64;
            }
            let at = self.pos as usize;
            if d.len() < at + buf.len() {
                d.resize(at + buf.len(), 0);
            }
            d[at..at + buf.len()].copy_from_slice(buf);
            self.pos += buf.len() as u64;
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for File {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            let len = self.data.borrow().len() as i64;
            let p = match to {
                SeekFrom::Start(n) => n as i64,
                SeekFrom::End(n) => len + n,
                SeekFrom::Current(n) => self.pos as i64 + n,
            };
            if p < 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"));
            }
            self.pos = p as u64;
            Ok(self.pos)
        }
    }

    #[derive(Clone, Debug, Default)]
    pub struct OpenOptions {
        write: bool,
        append: bool,
        truncate: bool,
        create: bool,
    }

    impl OpenOptions {
        pub fn new() -> OpenOptions {
            OpenOptions::default()
        }
        pub fn read(&mut self, _on: bool) -> &mut Self {
            self
        }
        pub fn write(&mut self, on: bool) -> &mut Self {
            self.write = on;
            self
        }
        pub fn append(&mut self, on: bool) -> &mut Self {
            self.append = on;
            self
        }
        pub fn truncate(&mut self, on: bool) -> &mut Self {
            self.truncate = on;
            self
        }
        pub fn create(&mut self, on: bool) -> &mut Self {
            self.create = on;
            self
        }
        pub fn create_new(&mut self, on: bool) -> &mut Self {
            self.create = on;
            self
        }
        pub fn open(&self, p: impl AsRef<Path>) -> io::Result<File> {
            let p = p.as_ref();
            match get(p) {
                Some(d) => {
                    if self.truncate {
                        d.borrow_mut().clear();
                    }
                    Ok(File { data: d, pos: 0, append: self.append })
                }
                None if self.create => {
                    put(p, Vec::new());
                    let mut f = File::open(p)?;
                    f.append = self.append;
                    Ok(f)
                }
                None => Err(not_found(p)),
            }
        }
    }

    // The page's way in and out.

    /// Puts a file there (an imported sample, an uploaded project).
    #[wasm_bindgen]
    pub fn lemon_fs_write(path: &str, data: &[u8]) {
        put(Path::new(path), data.to_vec());
    }

    #[wasm_bindgen]
    pub fn lemon_fs_mkdir(path: &str) {
        let _ = create_dir_all(path);
    }

    /// A file's bytes (`None`: not there).
    #[wasm_bindgen]
    pub fn lemon_fs_read(path: &str) -> Option<Vec<u8>> {
        read(path).ok()
    }

    #[wasm_bindgen]
    pub fn lemon_fs_remove(path: &str) {
        let _ = remove_file(path);
    }

    /// Every file under `dir`, one path per line, with its length and modification time (ms): "path\tlen\tms".
    #[wasm_bindgen]
    pub fn lemon_fs_list(dir: &str) -> String {
        let d = norm(Path::new(dir));
        FS.with(|fs| {
            let fs = fs.borrow();
            let mut out = String::new();
            for (k, n) in fs.files.iter().filter(|(k, _)| k.starts_with(&d)) {
                let ms = n.modified.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |t| t.as_millis());
                out.push_str(&format!("{}\t{}\t{}\n", k.display(), n.data.borrow().len(), ms));
            }
            out
        })
    }

    /// Counts the writes so far: the page saves what changed when it moves.
    #[wasm_bindgen]
    pub fn lemon_fs_version() -> f64 {
        FS.with(|fs| fs.borrow().version as f64)
    }
}
