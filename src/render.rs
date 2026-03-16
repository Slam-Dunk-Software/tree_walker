use crate::discover::Project;
use crate::extract::{extract_file, should_skip_dir, FileMap};
use chrono::Local;
use std::collections::HashSet;
use walkdir::WalkDir;

pub fn render(projects: &[Project]) -> String {
    let mut out = String::new();
    let mut project_blocks: Vec<(String, String)> = vec![];
    let mut total_files = 0usize;
    let mut total_symbols = 0usize;

    for project in projects {
        let (block, files, symbols) = render_project(project);
        if !block.is_empty() {
            project_blocks.push((project.name.clone(), block));
            total_files += files;
            total_symbols += symbols;
        }
    }

    let date = Local::now().format("%Y-%m-%d").to_string();
    out.push_str("# EPS Code Map\n");
    out.push_str(&format!(
        "_{} · {} projects · {} files · {} symbols_\n\n",
        date,
        project_blocks.len(),
        total_files,
        total_symbols,
    ));
    out.push_str("---\n\n");

    for (_, block) in project_blocks {
        out.push_str(&block);
    }

    out
}

fn render_project(project: &Project) -> (String, usize, usize) {
    let mut file_maps: Vec<FileMap> = WalkDir::new(&project.root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !should_skip_dir(&name)
        })
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| extract_file(e.path(), &project.root))
        .collect();

    if file_maps.is_empty() {
        return (String::new(), 0, 0);
    }

    file_maps.sort_by(|a, b| a.path.cmp(&b.path));

    // Collect unique languages
    let mut seen_langs: HashSet<&str> = HashSet::new();
    let langs: Vec<&str> = file_maps
        .iter()
        .filter_map(|f| seen_langs.insert(f.lang).then_some(f.lang))
        .collect();

    let total_symbols: usize = file_maps.iter().map(|f| f.symbols.len()).sum();
    let n_files = file_maps.len();

    let mut block = String::new();
    block.push_str(&format!("## {} `{}`\n", project.name, langs.join("/")));

    for fm in &file_maps {
        // Group symbols by kind, preserving insertion order for display
        let order = ["class", "module", "struct", "enum", "trait", "type", "fn", "def self", "def", "const"];
        let mut parts: Vec<String> = vec![];
        for kind in order {
            let names: Vec<&str> = fm
                .symbols
                .iter()
                .filter(|s| s.kind == kind)
                .map(|s| s.name.as_str())
                .collect();
            if !names.is_empty() {
                parts.push(format!("{}: {}", kind, names.join(", ")));
            }
        }
        block.push_str(&format!("`{}` — {}\n", fm.path, parts.join(" · ")));
    }

    block.push('\n');
    (block, n_files, total_symbols)
}
