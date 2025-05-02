// src/main.rs
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::map, // Keep terminated
    multi::separated_list1,
    sequence::{delimited, preceded, terminated},
};
use once_cell::sync::Lazy; // Use once_cell for regexes
use regex::Regex; // Add regex crate
use std::collections::{HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use strsim::levenshtein;
use thiserror::Error;

mod parser;

// --- Regex Definitions (using once_cell::sync::Lazy) ---
// Finds #if anywhere, captures condition. Need to check match pos later.
static IF_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#if\s+(.+)").expect("Invalid IF_RE regex"));
// Matches block #endif (start of line, allows trailing content like comments)
static BLOCK_ENDIF_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*#endif.*").expect("Invalid BLOCK_ENDIF_RE regex"));

#[derive(Error, Debug)]
pub(crate) enum ProcessorError {
    #[error("Mismatched #endif found at line {line_num} in {path}")]
    MismatchedEndif { line_num: usize, path: PathBuf },
    #[error("Mismatched #if without corresponding #endif at end of file {path}")]
    MismatchedIf { path: PathBuf },
    #[error("Failed to parse condition '{condition}' at line {line_num} in {path}: {reason}")]
    ConditionParse {
        condition: String,
        line_num: usize,
        path: PathBuf,
        reason: String,
    },
    #[error("I/O error processing file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Condition {
    Single(String),
    And(Vec<String>),
    Or(Vec<String>),
}

impl Condition {
    // Make methods used by tests crate-public
    pub(crate) fn evaluate(
        &self,
        flags: &HashSet<String>,
        used_flags: &mut HashSet<String>,
    ) -> bool {
        match self {
            Condition::Single(flag) => {
                used_flags.insert(flag.clone());
                flags.contains(flag)
            }
            Condition::And(terms) => {
                used_flags.extend(terms.iter().cloned());
                terms.iter().all(|term| flags.contains(term))
            }
            Condition::Or(terms) => {
                used_flags.extend(terms.iter().cloned());
                terms.iter().any(|term| flags.contains(term))
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Source directory with templates
    #[arg(long = "from", default_value = ".", value_name = "SRC_DIR")]
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

// --- Core Processing Logic ---
// Make function crate-public
pub(crate) fn process_content(
    reader: impl BufRead,
    file_path: &Path, // For error context
    flags: &HashSet<String>,
    used_flags: &mut HashSet<String>,
) -> Result<Vec<String>, ProcessorError> {
    let mut output = Vec::new();
    let mut include_stack: VecDeque<bool> = VecDeque::from([true]);
    let mut line_num = 0;

    for line_result in reader.lines() {
        line_num += 1;
        // Keep original line ending in the string for now if needed, though we add it back later
        let line = line_result.map_err(|e| ProcessorError::Io {
            path: file_path.to_path_buf(),
            source: e,
        })?;

        // --- Check for Block #endif ---
        if BLOCK_ENDIF_RE.is_match(&line) {
            if include_stack.len() > 1 {
                include_stack.pop_back();
            } else {
                // Mismatched #endif
                return Err(ProcessorError::MismatchedEndif {
                    line_num,
                    path: file_path.to_path_buf(),
                });
            }
            // Consume the #endif line entirely
            continue;
        }

        // --- Check for #if (Block or Inline) ---
        if let Some(captures) = IF_RE.captures(&line) {
            // Safely get match positions and condition string
            let full_match = captures.get(0).unwrap(); // The whole "#if ..." match
            let condition_str = captures.get(1).map_or("", |m| m.as_str()); // The condition part

            // Determine if it's block or inline based on what precedes the match
            let preceding_text = &line[..full_match.start()];
            let is_block_if = preceding_text.trim().is_empty();

            match parser::parse_condition_str(condition_str) {
                Ok(condition) => {
                    if is_block_if {
                        // --- Handle Block #if ---
                        let current_block_active = *include_stack.back().unwrap_or(&false);
                        let is_condition_met = condition.evaluate(flags, used_flags);
                        include_stack.push_back(current_block_active && is_condition_met);
                        // Consume the block #if line entirely
                        continue;
                    } else {
                        // --- Handle Inline #if ---
                        let content_before = preceding_text.trim_end(); // Content to potentially include
                        // let current_block_active = *include_stack.back().unwrap_or(&false);

                        // Include content_before only if the block is active AND the inline condition is true
                        if condition.evaluate(flags, used_flags) {
                            if !content_before.is_empty() {
                                // Avoid pushing empty strings
                                output.push(content_before.to_string());
                            }
                        }
                        // Regardless of the condition, the inline #if consumes the rest of the line.
                        // The include_stack is NOT affected.
                        continue; // Move to the next line
                    }
                }
                Err(reason) => {
                    // Failed to parse the condition string
                    return Err(ProcessorError::ConditionParse {
                        condition: condition_str.to_string(),
                        line_num,
                        path: file_path.to_path_buf(),
                        reason,
                    });
                }
            }
        }

        // --- Handle Regular Content Line (No Directives) ---
        // If we reach here, the line contains no directives (or was handled inline)
        // Check the current include state
        if *include_stack.back().unwrap_or(&false) {
            // If the block is active, add the entire line
            output.push(line);
        }
    }

    // Final check for mismatched #if at end of file
    if include_stack.len() != 1 {
        Err(ProcessorError::MismatchedIf {
            path: file_path.to_path_buf(),
        })
    } else {
        Ok(output)
    }
}

pub(crate) fn process_file(
    src_path: &Path,
    dest_path: &Path,
    flags: &HashSet<String>,
    used_flags: &mut HashSet<String>,
) -> Result<&'static str> {
    let file = File::open(src_path)
        .with_context(|| format!("Failed to open source file: {}", src_path.display()))?;
    let reader = BufReader::new(file);

    let processed_lines = process_content(reader, src_path, flags, used_flags) // Uses new logic
        .with_context(|| format!("Failed to process content of: {}", src_path.display()))?;

    if processed_lines.iter().all(|line| line.trim().is_empty()) {
        return Ok("skipped");
    }
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create destination directory: {}",
                parent.display()
            )
        })?;
    }
    let dest_file = File::create(dest_path)
        .with_context(|| format!("Failed to create destination file: {}", dest_path.display()))?;
    let mut writer = BufWriter::new(dest_file);
    for line in processed_lines {
        writeln!(writer, "{}", line).with_context(|| {
            format!(
                "Failed to write to destination file: {}",
                dest_path.display()
            )
        })?;
    }
    writer.flush().with_context(|| {
        format!(
            "Failed to flush writer for destination file: {}",
            dest_path.display()
        )
    })?;
    Ok("written")
}

pub(crate) fn find_closest_match<'a>(flag: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .min_by_key(|&&candidate| levenshtein(flag, candidate))
        .filter(|&&best_match| {
            let distance = levenshtein(flag, best_match);
            // Simple threshold: distance <= 2 or less than half the length
            let threshold = std::cmp::min(2, flag.len() / 2 + 1);
            distance <= threshold
        })
        .copied()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let flags: HashSet<String> = args.flags.into_iter().collect();
    let mut used_flags: HashSet<String> = HashSet::new();

    if !args.src_dir.is_dir() {
        return Err(anyhow!(
            "Source directory not found or not a directory: {}",
            args.src_dir.display()
        ));
    }
    // Ensure dest dir exists or create it
    if !args.dest_dir.exists() {
        fs::create_dir_all(&args.dest_dir).with_context(|| {
            format!(
                "Failed to create destination directory: {}",
                args.dest_dir.display()
            )
        })?;
    } else if !args.dest_dir.is_dir() {
        return Err(anyhow!(
            "Destination path exists but is not a directory: {}",
            args.dest_dir.display()
        ));
    }

    let mut files_processed = 0;
    let mut files_skipped = 0;
    let mut files_error = 0;

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
                continue; // Skip this file
            }
        };
        let dest_path = args.dest_dir.join(rel_path);

        if args.verbose {
            print!("Processing: {} ... ", rel_path.display());
            std::io::stdout().flush().ok();
        }

        match process_file(src_path, &dest_path, &flags, &mut used_flags) {
            Ok("skipped") => {
                files_skipped += 1;
                if args.verbose {
                    println!("{}", "Skipped (empty)".dimmed());
                }
            }
            Ok("written") => {
                files_processed += 1;
                if args.verbose {
                    println!("{}", "Written".green());
                }
            }
            Ok(other) => {
                if args.verbose {
                    println!("Unknown status: {}", other);
                }
            }
            Err(e) => {
                files_error += 1;
                if args.verbose {
                    println!();
                } // Newline after "Processing..."
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
            files_processed.to_string().green(),
            files_skipped.to_string().dimmed(),
            if files_error == 0 {
                "0".green()
            } else {
                files_error.to_string().red()
            }
        );
    }

    // --- Unused Flag Reporting ---
    let unused_flags: Vec<&String> = flags.difference(&used_flags).collect();
    if !unused_flags.is_empty() {
        println!("\n{}", "Unused flags:".yellow().bold());
        let used_flags_vec: Vec<&str> = used_flags.iter().map(String::as_str).collect();
        for &unused_flag in &unused_flags {
            let mut msg = format!(
                "  - Flag {} was provided but not used in any #if condition.",
                unused_flag.red(),
            );
            if !used_flags.contains(unused_flag) {
                if let Some(suggestion) = find_closest_match(unused_flag, &used_flags_vec) {
                    msg.push_str(&format!("\n  Did you mean {}?", suggestion.green()));
                }
            }
            println!("{}", msg);
        }
        let mut sorted_used: Vec<_> = used_flags.iter().collect();
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

// --- Add #[cfg(test)] mod tests; if using separate file ---
#[cfg(test)]
mod tests;
