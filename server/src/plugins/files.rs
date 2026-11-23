//! File explorer. Every path is confined to a root directory. No delete action, on purpose.

use super::{ActionInfo, Plugin, ToolError, ToolResult, info, opt_str, str_arg};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_READ: u64 = 200_000;
const MAX_HITS: usize = 100;

pub struct Files {
    root: PathBuf,
}

impl Files {
    pub fn new(root: &Path) -> Self {
        Files { root: fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()) }
    }

    /// Resolve `rel` inside root. Rejects `..`, and symlinks that leave root.
    fn safe(&self, rel: &str) -> Result<PathBuf, ToolError> {
        let mut p = self.root.clone();
        for c in Path::new(rel.trim_start_matches('/')).components() {
            match c {
                Component::Normal(n) => p.push(n),
                Component::CurDir => {}
                _ => return Err("path escapes root".into()),
            }
        }
        // Canonicalize the deepest existing ancestor to catch symlink escapes.
        let mut anc = p.as_path();
        while !anc.exists() {
            anc = anc.parent().ok_or("bad path")?;
        }
        let real = fs::canonicalize(anc).map_err(|e| e.to_string())?;
        if !real.starts_with(&self.root) {
            return Err("path escapes root".into());
        }
        Ok(p)
    }

    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.root).unwrap_or(p).display().to_string()
    }

    fn list(&self, args: &Value) -> ToolResult {
        let p = self.safe(opt_str(args, "path", "."))?;
        let mut entries = vec![];
        for e in fs::read_dir(&p).map_err(|e| e.to_string())?.flatten() {
            let md = e.metadata().ok();
            entries.push(json!({
                "name": e.file_name().to_string_lossy(),
                "dir": md.as_ref().is_some_and(|m| m.is_dir()),
                "size": md.filter(|m| m.is_file()).map(|m| m.len()),
            }));
        }
        entries.sort_by_key(|e| e["name"].as_str().unwrap_or("").to_string());
        Ok(json!({ "root": self.root, "path": self.rel(&p), "entries": entries }))
    }

    fn read(&self, args: &Value) -> ToolResult {
        let path = str_arg(args, "path")?;
        let p = self.safe(path)?;
        if !p.is_file() {
            return Err("not a file".into());
        }
        use std::io::Read;
        let mut buf = vec![];
        fs::File::open(&p)
            .and_then(|f| f.take(MAX_READ).read_to_end(&mut buf))
            .map_err(|e| e.to_string())?;
        Ok(json!({ "path": path, "content": String::from_utf8_lossy(&buf) }))
    }

    /// Returns the old text (if any, up to 200 KB) so the write can be undone.
    fn write(&self, args: &Value) -> ToolResult {
        let p = self.safe(str_arg(args, "path")?)?;
        let content = str_arg(args, "content")?;
        let previous = if p.is_file() {
            let old = fs::read(&p).map_err(|e| e.to_string())?;
            if old.len() as u64 > MAX_READ { None } else { Some(String::from_utf8_lossy(&old).into_owned()) }
        } else {
            None
        };
        let existed = p.exists();
        if let Some(d) = p.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        fs::write(&p, content).map_err(|e| e.to_string())?;
        Ok(json!({ "written": content.len(), "existed": existed, "previous": previous }))
    }

    /// Undo of `write`: put the old text back. A new file cannot be removed (no delete), so it is emptied.
    fn restore(&self, args: &Value) -> ToolResult {
        let path = args["args"]["path"].as_str().ok_or("missing args.path")?;
        let p = self.safe(path)?;
        match (args["result"]["existed"].as_bool(), args["result"]["previous"].as_str()) {
            (Some(true), Some(prev)) => fs::write(&p, prev).map_err(|e| e.to_string())?,
            (Some(true), None) => return Err("the old file was too big to keep; cannot undo".into()),
            _ => fs::write(&p, "").map_err(|e| e.to_string())?,
        }
        Ok(json!({ "restored": path }))
    }

    /// Undo of `mkdir`: remove the folder only if it is still empty.
    fn rmdir_empty(&self, args: &Value) -> ToolResult {
        let path = args["args"]["path"].as_str().ok_or("missing args.path")?;
        fs::remove_dir(self.safe(path)?).map_err(|_| "folder is not empty or is gone".to_string())?;
        Ok(json!({ "removed": path }))
    }

    /// Undo of `move`: move it back.
    fn move_back(&self, args: &Value) -> ToolResult {
        let a = &args["args"];
        self.mv(&json!({ "src": a["dst"], "dst": a["src"] }))
    }

    fn mkdir(&self, args: &Value) -> ToolResult {
        let p = self.safe(str_arg(args, "path")?)?;
        if p.exists() {
            return Err("already exists".into());
        }
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
        Ok(json!({ "ok": true }))
    }

    fn mv(&self, args: &Value) -> ToolResult {
        let (s, d) = (self.safe(str_arg(args, "src")?)?, self.safe(str_arg(args, "dst")?)?);
        if d.exists() {
            return Err("destination exists".into());
        }
        fs::rename(s, d).map_err(|e| e.to_string())?;
        Ok(json!({ "ok": true }))
    }

    fn search(&self, args: &Value) -> ToolResult {
        let q = str_arg(args, "query")?.to_lowercase();
        let start = self.safe(opt_str(args, "path", "."))?;
        let (mut hits, mut stack) = (vec![], vec![start]);
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                // Do not follow symlinks while walking.
                let ft = match e.file_type() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if ft.is_dir() {
                    stack.push(p);
                } else if e.file_name().to_string_lossy().to_lowercase().contains(&q) {
                    hits.push(self.rel(&p));
                    if hits.len() >= MAX_HITS {
                        return Ok(json!({ "hits": hits, "truncated": true }));
                    }
                }
            }
        }
        Ok(json!({ "hits": hits, "truncated": false }))
    }
}

impl Plugin for Files {
    fn name(&self) -> &'static str {
        "files"
    }
    fn actions(&self) -> BTreeMap<&'static str, ActionInfo> {
        BTreeMap::from([
            ("list", info("List a directory", &[("path", "dir, relative to root")])),
            ("read", info("Read a text file", &[("path", "file")])),
            ("write", info("Write a text file", &[("path", "file"), ("content", "text")])),
            ("mkdir", info("Make a folder", &[("path", "dir")])),
            ("move", info("Move or rename", &[("src", "from"), ("dst", "to")])),
            ("search", info("Find files by name part", &[("query", "text"), ("path", "start dir")])),
            ("restore", info("Undo a write", &[])),
            ("rmdir_empty", info("Undo a mkdir", &[])),
            ("move_back", info("Undo a move", &[])),
        ])
    }
    fn call(&self, action: &str, args: &Value) -> Option<ToolResult> {
        Some(match action {
            "list" => self.list(args),
            "read" => self.read(args),
            "write" => self.write(args),
            "mkdir" => self.mkdir(args),
            "move" => self.mv(args),
            "search" => self.search(args),
            "restore" => self.restore(args),
            "rmdir_empty" => self.rmdir_empty(args),
            "move_back" => self.move_back(args),
            _ => return None,
        })
    }
}
