use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Project {
    pub name: String,
    pub root: PathBuf,
}

#[derive(Deserialize)]
struct ServicesFile {
    services: Option<HashMap<String, ServiceEntry>>,
}

#[derive(Deserialize)]
struct ServiceEntry {
    dir: Option<String>,
}

pub fn find_all(use_epc: bool, extra_dirs: &[PathBuf]) -> Result<Vec<Project>> {
    let mut projects = vec![];

    if use_epc {
        projects.extend(from_epc()?);
    }

    // Scan personal-projects for eps.toml files (catches non-running projects too)
    let personal = home().join("Documents/personal-projects");
    if personal.exists() {
        projects.extend(from_eps_toml_scan(&personal));
    }

    for dir in extra_dirs {
        if dir.exists() {
            projects.push(Project {
                name: dir_name(dir),
                root: dir.clone(),
            });
        }
    }

    Ok(projects)
}

fn from_epc() -> Result<Vec<Project>> {
    let path = home().join(".epc/services.toml");
    if !path.exists() {
        return Ok(vec![]);
    }

    let content = std::fs::read_to_string(&path)?;
    let Ok(file) = toml::from_str::<ServicesFile>(&content) else {
        return Ok(vec![]);
    };

    Ok(file
        .services
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, entry)| {
            let dir = PathBuf::from(entry.dir?);
            dir.exists().then_some(Project { name, root: dir })
        })
        .collect())
}

fn from_eps_toml_scan(root: &Path) -> Vec<Project> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            (path.is_dir() && path.join("eps.toml").exists()).then(|| Project {
                name: dir_name(&path),
                root: path,
            })
        })
        .collect()
}

fn home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn dir_name(p: &Path) -> String {
    p.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
