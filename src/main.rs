use anyhow::{Context, Result, bail};
use clap::Parser;
use colored::*;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use strsim::levenshtein;

// --- Condition type and parser ---

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Condition {
    Single(String),
    And(Vec<String>),
    Or(Vec<String>),
}

impl Condition {
    pub(crate) fn evaluate(&self, flags: &HashSet<String>) -> (bool, HashSet<String>) {
        match self {
            Condition::Single(flag) => {
                (flags.contains(flag), HashSet::from([flag.clone()]))
            }
            Condition::And(terms) => {
                let used: HashSet<String> = terms.iter().cloned().collect();
                (terms.iter().all(|t| flags.contains(t)), used)
            }
            Condition::Or(terms) => {
                let used: HashSet<String> = terms.iter().cloned().collect();
                (terms.iter().any(|t| flags.contains(t)), used)
            }
        }
    }
}

pub(crate) fn parse_condition(s: &str) -> Result<Condition> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("(and ").and_then(|s| s.strip_suffix(')')) {
        Ok(Condition::And(inner.split_whitespace().map(Into::into).collect()))
    } else if let Some(inner) = s.strip_prefix("(or ").and_then(|s| s.strip_suffix(')')) {
        Ok(Condition::Or(inner.split_whitespace().map(Into::into).collect()))
    } else if !s.is_empty() && s.chars().all(|c| !c.is_whitespace() && c != '(' && c != ')') {
        Ok(Condition::Single(s.into()))
    } else {
        bail!("invalid condition: '{s}'")
    }
}

// --- CLI args ---

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Source directory with templates
    #[arg(long = "from", default_value = "./templates", value_name = "SRC_DIR")]
    src_dir: PathBuf,

    /// Destination directory
    #[arg(long = "to", default_value = ".", value_name = "DEST_DIR")]
    dest_dir: PathBuf,

    /// Print which files were processed or skipped
    #[arg(long, short)]
    verbose: bool,

    /// Flags like `clj devshell` to include conditionals
    #[arg(required = true, num_args = 1..)]
    flags: Vec<String>,
}

// --- Core processing ---

pub(crate) fn process_content(
    reader: impl BufRead,
    file_path: &Path,
    flags: &HashSet<String>,
) -> Result<(Vec<String>, HashSet<String>)> {
    let (output, used_flags, stack) = reader
        .lines()
        .enumerate()
        .try_fold(
            (Vec::new(), HashSet::<String>::new(), vec![true]),
            |(mut output, mut used_flags, mut stack), (idx, line_result)| {
                let line_num = idx + 1;
                let line = line_result.with_context(|| {
                    format!("I/O error at line {line_num} in {}", file_path.display())
                })?;

                // Block #endif
                if line.trim_start().starts_with("#endif") {
                    if stack.len() > 1 {
                        stack.pop();
                    } else {
                        bail!(
                            "mismatched #endif at line {line_num} in {}",
                            file_path.display()
                        );
                    }
                    return Ok((output, used_flags, stack));
                }

                // #if (block or inline)
                if let Some(pos) = line.find("#if ") {
                    let condition_str = &line[pos + 4..];
                    let prefix = &line[..pos];
                    let is_block = prefix.trim().is_empty();

                    let condition = parse_condition(condition_str).with_context(|| {
                        format!(
                            "failed to parse condition '{condition_str}' at line {line_num} in {}",
                            file_path.display()
                        )
                    })?;

                    let (met, cond_used) = condition.evaluate(flags);
                    used_flags.extend(cond_used);

                    if is_block {
                        let parent_active = *stack.last().unwrap_or(&false);
                        stack.push(parent_active && met);
                    } else if met {
                        let trimmed = prefix.trim_end();
                        if !trimmed.is_empty() {
                            output.push(trimmed.to_string());
                        }
                    }
                    return Ok((output, used_flags, stack));
                }

                // Regular content line
                if *stack.last().unwrap_or(&false) {
                    output.push(line);
                }

                Ok((output, used_flags, stack))
            },
        )?;

    if stack.len() != 1 {
        bail!(
            "mismatched #if without #endif in {}",
            file_path.display()
        );
    }

    Ok((output, used_flags))
}

pub(crate) fn process_file(
    src_path: &Path,
    dest_path: &Path,
    flags: &HashSet<String>,
) -> Result<(Option<PathBuf>, HashSet<String>)> {
    let file = File::open(src_path)
        .with_context(|| format!("failed to open: {}", src_path.display()))?;

    let (lines, used_flags) = process_content(BufReader::new(file), src_path, flags)?;

    if lines.iter().all(|l| l.trim().is_empty()) {
        return Ok((None, used_flags));
    }

    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create dir: {}", parent.display()))?;
    }

    let mut writer = BufWriter::new(
        File::create(dest_path)
            .with_context(|| format!("failed to create: {}", dest_path.display()))?,
    );
    for line in &lines {
        writeln!(writer, "{line}")
            .with_context(|| format!("failed to write: {}", dest_path.display()))?;
    }
    writer
        .flush()
        .with_context(|| format!("failed to flush: {}", dest_path.display()))?;

    Ok((Some(dest_path.to_path_buf()), used_flags))
}

pub(crate) fn find_closest_match<'a>(flag: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .min_by_key(|&&candidate| levenshtein(flag, candidate))
        .filter(|&&best_match| {
            let distance = levenshtein(flag, best_match);
            let threshold = std::cmp::min(2, flag.len() / 2 + 1);
            distance <= threshold
        })
        .copied()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let flags: HashSet<String> = args.flags.into_iter().collect();
    let mut all_used_flags: HashSet<String> = HashSet::new();

    if !args.src_dir.is_dir() {
        bail!(
            "source directory not found or not a directory: {}",
            args.src_dir.display()
        );
    }

    if !args.dest_dir.exists() {
        fs::create_dir_all(&args.dest_dir).with_context(|| {
            format!(
                "failed to create destination directory: {}",
                args.dest_dir.display()
            )
        })?;
    } else if !args.dest_dir.is_dir() {
        bail!(
            "destination path exists but is not a directory: {}",
            args.dest_dir.display()
        );
    }

    let mut files_written = 0u32;
    let mut files_skipped = 0u32;
    let mut files_error = 0u32;

    for entry in walkdir::WalkDir::new(&args.src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let src_path = entry.path();
        let rel_path = match src_path.strip_prefix(&args.src_dir) {
            Ok(p) => p,
            Err(_) => {
                eprintln!(
                    "{}",
                    format!(
                        "Warning: Could not determine relative path for {}",
                        src_path.display()
                    )
                    .yellow()
                );
                continue;
            }
        };
        let dest_path = args.dest_dir.join(rel_path);

        if args.verbose {
            print!("Processing: {} ... ", rel_path.display());
            std::io::stdout().flush().ok();
        }

        match process_file(src_path, &dest_path, &flags) {
            Ok((None, used)) => {
                all_used_flags.extend(used);
                files_skipped += 1;
                if args.verbose {
                    println!("{}", "Skipped (empty)".dimmed());
                }
            }
            Ok((Some(_), used)) => {
                all_used_flags.extend(used);
                files_written += 1;
                if args.verbose {
                    println!("{}", "Written".green());
                }
            }
            Err(e) => {
                files_error += 1;
                if args.verbose {
                    println!();
                }
                eprintln!(
                    "{}",
                    format!("Error processing {}: {:?}", rel_path.display(), e).red()
                );
            }
        }
    }

    if args.verbose {
        println!(
            "\nSummary: {} written, {} skipped, {} errors.",
            files_written.to_string().green(),
            files_skipped.to_string().dimmed(),
            if files_error == 0 {
                "0".green()
            } else {
                files_error.to_string().red()
            }
        );
    }

    // Unused flag reporting
    let unused_flags: Vec<&String> = flags.difference(&all_used_flags).collect();
    if !unused_flags.is_empty() {
        println!("\n{}", "Unused flags:".yellow().bold());
        let used_flags_vec: Vec<&str> = all_used_flags.iter().map(String::as_str).collect();
        for &unused_flag in &unused_flags {
            let mut msg = format!(
                "  - Flag {} was provided but not used in any #if condition.",
                unused_flag.red(),
            );
            if let Some(suggestion) = find_closest_match(unused_flag, &used_flags_vec) {
                msg.push_str(&format!("\n  Did you mean {}?", suggestion.green()));
            }
            println!("{msg}");
        }
        let mut sorted_used: Vec<_> = all_used_flags.iter().collect();
        sorted_used.sort_unstable();
        if !sorted_used.is_empty() {
            println!("\n{}", "Available Flags:".dimmed());
            print!("  ");
            for (i, flag) in sorted_used.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{}", flag.cyan());
            }
            println!();
        }
    }

    if files_error > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests;
