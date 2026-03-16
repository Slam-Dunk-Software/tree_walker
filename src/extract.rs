use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: &'static str,
    pub name: String,
}

#[derive(Debug)]
pub struct FileMap {
    pub path: String,
    pub lang: &'static str,
    pub symbols: Vec<Symbol>,
}

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    "vendor",
    "tmp",
    "log",
    "dist",
    "build",
    "coverage",
    ".next",
    ".cache",
    "db",               // Rails migrations/schema
    "public",           // Rails/Node static assets
    "SourcePackages",   // Xcode/Swift package checkouts
    "Pods",             // CocoaPods
    "checkouts",        // SPM checkouts
    "examples",         // library examples
    "bindings",         // FFI bindings
];

pub fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.iter().any(|s| *s == name)
}

pub fn extract_file(path: &Path, root: &Path) -> Option<FileMap> {
    let ext = path.extension()?.to_str()?;
    let rel = path
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .into_owned();

    // Skip test/spec files — too noisy
    if rel.contains("/test/") || rel.contains("/spec/") || rel.contains("_test.rs") {
        return None;
    }

    // Skip minified files
    if rel.ends_with(".min.js") || rel.ends_with(".min.ts") {
        return None;
    }

    let source = std::fs::read_to_string(path).ok()?;
    if source.is_empty() || source.len() > 256_000 {
        return None;
    }

    let (lang_name, symbols) = match ext {
        "rs" => ("Rust", extract_rust(&source)?),
        "rb" => ("Ruby", extract_ruby(&source)?),
        "js" | "mjs" => ("JS", extract_js(&source)?),
        "ts" => ("TS", extract_ts(&source)?),
        "tsx" => ("TSX", extract_tsx(&source)?),
        _ => return None,
    };

    if symbols.is_empty() {
        return None;
    }

    Some(FileMap {
        path: rel,
        lang: lang_name,
        symbols,
    })
}

// --- helpers ---

fn parse(lang: Language, source: &str) -> Option<(Tree, Vec<u8>)> {
    let mut parser = Parser::new();
    parser.set_language(lang).ok()?;
    let bytes = source.as_bytes().to_vec();
    let tree = parser.parse(&bytes, None)?;
    Some((tree, bytes))
}

fn query_names(
    lang: Language,
    query_str: &str,
    cap_name: &str,
    source: &[u8],
    tree: &Tree,
) -> Vec<String> {
    let Ok(query) = Query::new(lang, query_str) else {
        return vec![];
    };
    let Some(idx) = query.capture_index_for_name(cap_name) else {
        return vec![];
    };
    let mut cursor = QueryCursor::new();
    let mut results = vec![];
    for m in cursor.matches(&query, tree.root_node(), source) {
        for cap in m.captures {
            if cap.index == idx {
                if let Ok(text) = cap.node.utf8_text(source) {
                    results.push(text.to_string());
                }
            }
        }
    }
    results
}

fn collect(lang: Language, queries: &[(&str, &str, &'static str)], source: &[u8], tree: &Tree) -> Vec<Symbol> {
    let mut symbols = vec![];
    for (q, cap, kind) in queries {
        for name in query_names(lang, q, cap, source, tree) {
            symbols.push(Symbol { kind, name });
        }
    }
    symbols
}

// --- language extractors ---

fn extract_rust(source: &str) -> Option<Vec<Symbol>> {
    let lang = tree_sitter_rust::language();
    let (tree, bytes) = parse(lang, source)?;
    Some(collect(
        lang,
        &[
            ("(function_item (visibility_modifier) name: (identifier) @name)", "name", "fn"),
            ("(struct_item (visibility_modifier) name: (type_identifier) @name)", "name", "struct"),
            ("(enum_item (visibility_modifier) name: (type_identifier) @name)", "name", "enum"),
            ("(trait_item (visibility_modifier) name: (type_identifier) @name)", "name", "trait"),
            ("(type_item (visibility_modifier) name: (type_identifier) @name)", "name", "type"),
        ],
        &bytes,
        &tree,
    ))
}

fn extract_ruby(source: &str) -> Option<Vec<Symbol>> {
    let lang = tree_sitter_ruby::language();
    let (tree, bytes) = parse(lang, source)?;
    Some(collect(
        lang,
        &[
            ("(class name: (constant) @name)", "name", "class"),
            ("(module name: (constant) @name)", "name", "module"),
            ("(method name: (identifier) @name)", "name", "def"),
            ("(singleton_method name: (identifier) @name)", "name", "def self"),
        ],
        &bytes,
        &tree,
    ))
}

fn extract_js(source: &str) -> Option<Vec<Symbol>> {
    extract_js_like(tree_sitter_javascript::language(), source)
}

fn extract_ts(source: &str) -> Option<Vec<Symbol>> {
    extract_js_like(tree_sitter_typescript::language_typescript(), source)
}

fn extract_tsx(source: &str) -> Option<Vec<Symbol>> {
    extract_js_like(tree_sitter_typescript::language_tsx(), source)
}

fn extract_js_like(lang: Language, source: &str) -> Option<Vec<Symbol>> {
    let (tree, bytes) = parse(lang, source)?;
    let mut symbols = collect(
        lang,
        &[
            // ESM exports
            ("(export_statement declaration: (function_declaration name: (identifier) @name))", "name", "fn"),
            ("(export_statement declaration: (class_declaration name: (identifier) @name))", "name", "class"),
            ("(export_statement declaration: (lexical_declaration (variable_declarator name: (identifier) @name)))", "name", "const"),
            // Top-level (for CJS / files without explicit exports)
            ("(function_declaration name: (identifier) @name)", "name", "fn"),
            ("(class_declaration name: (identifier) @name)", "name", "class"),
        ],
        &bytes,
        &tree,
    );

    // Deduplicate by name (exports + top-level queries overlap)
    let mut seen = std::collections::HashSet::new();
    symbols.retain(|s| seen.insert(s.name.clone()));
    Some(symbols)
}
