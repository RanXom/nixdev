use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};

use crate::nix;
use crate::util::{fuzzy_score, now_secs};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub attr: String,
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct Index {
    pub rev: String,
    pub system: String,
    pub built: u64,
    pub entries: Vec<Entry>,
}

impl Index {
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Entry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.entries.iter().take(limit).collect();
        }
        let mut hits: Vec<(i32, &Entry)> = self
            .entries
            .iter()
            .filter_map(|entry| score(entry, &query).map(|s| (s, entry)))
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.attr.cmp(&b.1.attr)));
        hits.into_iter().take(limit).map(|(_, e)| e).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn score(entry: &Entry, query: &str) -> Option<i32> {
    let attr = entry.attr.to_lowercase();
    let name = entry.name.to_lowercase();
    let description = entry.description.to_lowercase();

    if attr == query {
        return Some(10_000);
    }
    let mut best: Option<i32> = None;
    if attr.ends_with(&format!(".{query}")) {
        best = Some(9_000);
    }
    if name == query || name == format!("{query}-{}", entry.version) {
        best = best.max(Some(8_500));
    }
    if let Some(score) = fuzzy_score(&attr, query) {
        best = best.max(Some(score + 500));
    }
    if name.contains(query) {
        best = best.max(Some(1_200));
    }
    if description.contains(query) {
        best = best.max(Some(600));
    }
    best
}

pub fn cache_dir() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("nixdev");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("nixdev");
    }
    env::temp_dir().join(format!(
        "nixdev-cache-{}",
        env::var("USER").unwrap_or_else(|_| "user".into())
    ))
}

pub fn index_path(rev: &str, system: &str) -> PathBuf {
    let short: String = rev.chars().take(16).collect();
    cache_dir()
        .join("index")
        .join(format!("{system}-{short}.json.gz"))
}

pub fn load(rev: &str, system: &str) -> Option<Index> {
    let path = index_path(rev, system);
    let bytes = fs::read(path).ok()?;
    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut text = String::new();
    decoder.read_to_string(&mut text).ok()?;
    let index: Index = serde_json::from_str(&text).ok()?;
    if index.rev != rev || index.system != system || index.entries.is_empty() {
        return None;
    }
    Some(index)
}

pub fn save(index: &Index) -> Result<PathBuf, String> {
    let path = index_path(&index.rev, &index.system);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create `{}`: {err}", parent.display()))?;
    }
    let json = serde_json::to_vec(index).map_err(|err| format!("could not encode index: {err}"))?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&json)
        .map_err(|err| format!("could not compress index: {err}"))?;
    let bytes = encoder
        .finish()
        .map_err(|err| format!("could not compress index: {err}"))?;

    let temp = path.with_extension(format!("gz.tmp.{}", std::process::id()));
    fs::write(&temp, bytes).map_err(|err| format!("could not write index: {err}"))?;
    fs::rename(&temp, &path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("could not install index: {err}")
    })?;
    Ok(path)
}

pub fn entry_from_json(attr: String, value: &serde_json::Value) -> Entry {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };
    let name = text("name")
        .or_else(|| text("pname"))
        .unwrap_or_else(|| attr.clone());
    Entry {
        attr,
        name,
        version: text("version").unwrap_or_default(),
        description: text("description").unwrap_or_default(),
    }
}

pub fn build(rev: &str, system: &str) -> Result<Index, String> {
    let args = nix::nix_args(&["search", "nixpkgs", "^", "--json"]);
    let out = nix::run(&nix::nix_bin(), &args, None, Duration::from_secs(900))
        .map_err(|err| format!("could not search nixpkgs: {err}"))?;
    let raw: serde_json::Value = serde_json::from_str(&out.stdout)
        .map_err(|err| format!("could not parse nixpkgs search output: {err}"))?;
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    if let Some(map) = raw.as_object() {
        for (key, value) in map {
            let attr = nix::attr_from_key(key, system);
            if attr.is_empty() || !seen.insert(attr.clone()) {
                continue;
            }
            entries.push(entry_from_json(attr, value));
        }
    }
    if entries.is_empty() {
        return Err("nixpkgs search returned no packages".into());
    }
    entries.sort_by(|a, b| a.attr.cmp(&b.attr));
    Ok(Index {
        rev: rev.to_string(),
        system: system.to_string(),
        built: now_secs(),
        entries,
    })
}

pub fn load_or_build(rev: &str, system: &str) -> Result<Index, String> {
    if let Some(index) = load(rev, system) {
        return Ok(index);
    }
    let index = build(rev, system)?;
    let _ = save(&index);
    Ok(index)
}

pub fn is_cached(rev: &str, system: &str) -> bool {
    index_path(rev, system).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(attr: &str, name: &str, description: &str) -> Entry {
        Entry {
            attr: attr.into(),
            name: name.into(),
            version: "1.0".into(),
            description: description.into(),
        }
    }

    #[test]
    fn ranks_exact_and_prefixed_matches_first() {
        let index = Index {
            rev: "test".into(),
            system: "x86_64-linux".into(),
            built: 0,
            entries: vec![
                entry("yyjson", "yyjson", "fast JSON library"),
                entry("python3Packages.yyjson", "yyjson", "python bindings"),
                entry("dbus", "dbus", "message bus"),
            ],
        };
        let hits = index.search("yyjson", 10);
        assert_eq!(hits[0].attr, "yyjson");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn searches_descriptions() {
        let index = Index {
            rev: "test".into(),
            system: "x86_64-linux".into(),
            built: 0,
            entries: vec![entry("dbus", "dbus", "message bus daemon")],
        };
        assert_eq!(index.search("message bus", 5).len(), 1);
    }

    #[test]
    fn gzip_round_trip() {
        let dir = std::env::temp_dir().join(format!("nixdev-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        unsafe { env::set_var("XDG_CACHE_HOME", &dir) };
        let index = Index {
            rev: "abc".into(),
            system: "x86_64-linux".into(),
            built: 1,
            entries: vec![entry("yyjson", "yyjson", "json")],
        };
        save(&index).unwrap();
        let loaded = load("abc", "x86_64-linux").unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(load("other", "x86_64-linux").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
