use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod config;

use config::Config;

#[derive(Parser, Debug)]
#[command(name = "rocksdb-tui")]
#[command(about = "A TUI browser for RocksDB databases")]
struct Args {
    /// Path to RocksDB database directory
    #[arg(long, required = true)]
    db: PathBuf,

    /// Path to parser config file (TOML)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Path for secondary instance (default: /tmp/rocksdb-tui-<hash>)
    #[arg(long)]
    secondary: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = match &args.config {
        Some(path) => Config::load(path)?,
        None => Config::default(),
    };

    println!("Database: {:?}", args.db);
    println!("Config: {:?}", config);

    Ok(())
}
