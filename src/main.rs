mod discover;
mod extract;
mod render;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tree_walker", about = "EPS codebase index — extracts public symbols from all your projects")]
struct Args {
    /// Additional directories to index (beyond EPC + eps.toml discovery)
    #[arg(short, long)]
    dir: Vec<PathBuf>,

    /// Output file path (omit for stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Skip EPC services.toml discovery
    #[arg(long)]
    no_epc: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut projects = discover::find_all(!args.no_epc, &args.dir)?;
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects.dedup_by_key(|p| p.root.clone());

    let map = render::render(&projects);

    match args.output {
        Some(path) => {
            std::fs::write(&path, &map)?;
            eprintln!("wrote {}", path.display());
        }
        None => print!("{map}"),
    }

    Ok(())
}
