use clap::Parser;
use anyhow::{Context, Result};

#[derive(Parser)]
struct Cli {
    from: std::path::PathBuf,
    to: std::path::PathBuf,
    // flags: ,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let content = std::fs::read_to_string(&args.from)
        .with_context(|| format!("Could not read file `{}`", args.from.display()))?;

    println!("from: {:?}, to: {:?}", args.from, args.to);

    for line in content.lines() {
        if line.contains("#if") || line.contains("#endif") {
            println!("{}", line);
        }
    }

    Ok(())
}
