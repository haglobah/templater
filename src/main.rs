#[cfg(test)]
mod tests;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::{map, recognize, rest},
    multi::separated_list1,
    sequence::{delimited, preceded, terminated, tuple},
};
use std::collections::{HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use strsim::levenshtein;
use thiserror::Error;

// --- Custom Error Types ---
#[derive(Error, Debug)]
enum ProcessorError {
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

// --- Data Structures for Parsed Conditions ---
#[derive(Debug, PartialEq, Clone)]
enum Condition {
    Single(String),
    And(Vec<String>),
    Or(Vec<String>),
}

impl Condition {
    /// Evaluates the parsed condition against the provided flags.
    /// Also collects all flags encountered in the condition into `used_flags`.
    fn evaluate(&self, flags: &HashSet<String>, used_flags: &mut HashSet<String>) -> bool {
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

    /// Extracts all flag names mentioned in the condition.
    fn mentioned_flags(&self) -> Vec<String> {
        match self {
            Condition::Single(flag) => vec![flag.clone()],
            Condition::And(terms) | Condition::Or(terms) => terms.clone(),
        }
    }
}

// --- Parser Logic (`nom`) ---
mod parser {
    use super::*; // Import necessary items from outer scope

    // Represents the outcome of parsing a single line
    #[derive(Debug, PartialEq)]
    pub(super) enum LineParseResult<'a> {
        If(Condition),
        Endif,
        Content(&'a str), // The actual content line
    }

    // Basic identifier/flag parser (non-whitespace, non-parenthesis)
    fn identifier(input: &str) -> IResult<&str, &str> {
        take_while1(|c: char| !c.is_whitespace() && c != '(' && c != ')')(input)
    }

    // Parser for "(and flag1 flag2 ...)"
    fn parse_and(input: &str) -> IResult<&str, Condition> {
        map(
            delimited(
                tag("(and"),
                preceded(multispace1, separated_list1(multispace1, identifier)),
                preceded(multispace0, char(')')),
            ),
            |flags: Vec<&str>| Condition::And(flags.into_iter().map(String::from).collect()),
        )(input)
    }

    // Parser for "(or flag1 flag2 ...)"
    fn parse_or(input: &str) -> IResult<&str, Condition> {
        map(
            delimited(
                tag("(or"),
                preceded(multispace1, separated_list1(multispace1, identifier)),
                preceded(multispace0, char(')')),
            ),
            |flags: Vec<&str>| Condition::Or(flags.into_iter().map(String::from).collect()),
        )(input)
    }

    // Parser for a single flag condition
    fn parse_single(input: &str) -> IResult<&str, Condition> {
        map(identifier, |flag| Condition::Single(flag.to_string()))(input)
    }

    // Parser for any valid condition
    fn parse_condition(input: &str) -> IResult<&str, Condition> {
        alt((parse_and, parse_or, parse_single))(input)
    }

    // Parser for "#if condition" line
    fn parse_if_directive(input: &str) -> IResult<&str, LineParseResult> {
        map(
            preceded(
                tuple((multispace0, tag("#if"), multispace1)),
                // Important: consume trailing whitespace/newline after condition
                terminated(parse_condition, multispace0),
            ),
            LineParseResult::If,
        )(input)
    }

    // Parser for "#endif" line
    fn parse_endif_directive(input: &str) -> IResult<&str, LineParseResult> {
        map(
            // Ensure the whole line is matched (or just whitespace after #endif)
            recognize(tuple((multispace0, tag("#endif"), multispace0))),
            |_| LineParseResult::Endif,
        )(input)
    }

    // Top-level line parser
    // Tries to parse #if, then #endif. If both fail, it's content.
    pub(super) fn parse_line(input: &str) -> IResult<&str, LineParseResult> {
        alt((
            parse_if_directive,
            parse_endif_directive,
            map(rest, LineParseResult::Content), // If others fail, take the rest as content
        ))(input)
    }
} // end mod parser

// --- Argument Parsing ---
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

/// Processes lines from a reader based on conditional blocks and flags.
fn process_content(
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
        let line = line_result.map_err(|e| ProcessorError::Io {
            path: file_path.to_path_buf(),
            source: e,
        })?;

        match parser::parse_line(&line) {
            Ok((_, parse_result)) => match parse_result {
                parser::LineParseResult::If(condition) => {
                    let current_block_active = *include_stack.back().unwrap_or(&false); // Should always have initial `true`
                    let is_condition_met = condition.evaluate(flags, used_flags);
                    include_stack.push_back(current_block_active && is_condition_met);
                }
                parser::LineParseResult::Endif => {
                    if include_stack.len() > 1 {
                        include_stack.pop_back();
                    } else {
                        return Err(ProcessorError::MismatchedEndif {
                            line_num,
                            path: file_path.to_path_buf(),
                        });
                    }
                }
                parser::LineParseResult::Content(content_str) => {
                    if *include_stack.back().unwrap_or(&false) {
                        // Only push the relevant content part if nom didn't consume the whole line
                        // In our current parser setup, `Content` gets the *whole* original line.
                        output.push(content_str.to_string());
                    }
                }
            },
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => {
                // If parse_line fails, it should only be due to a malformed #if condition,
                // as Content covers everything else. Let's try to extract the condition part.
                // A simpler approach: treat any parse failure on non-empty lines as potential error
                if !line.trim().is_empty() {
                    // Attempt to find the #if part to report it
                    let relevant_slice: &str = line
                        .trim_start()
                        .strip_prefix("#if") // Returns Option<&str> containing the part *after* "#if"
                        .unwrap_or(&line); // If no "#if", use the original line slice (&str via Deref<Target=str>)

                    // Trim whitespace from the chosen slice and own it.
                    let condition_part: String = relevant_slice.trim().to_owned();
                    // let condition_part: &String = line.trim_start().strip_prefix("#if").map_or(&line, |s| &(s.trim().to_owned()));
                    return Err(ProcessorError::ConditionParse {
                        condition: condition_part.to_string(),
                        line_num,
                        path: file_path.to_path_buf(),
                        reason: format!("nom parser error: {:?}", e.code), // Provide nom error code
                    });
                }
                // Otherwise, likely an empty line or just whitespace, treat as content (if active)
                else if *include_stack.back().unwrap_or(&false) {
                    output.push(line);
                }
            }
            Err(nom::Err::Incomplete(_)) => {
                // Should not happen when reading complete lines
                return Err(ProcessorError::ConditionParse {
                    condition: line.to_string(),
                    line_num,
                    path: file_path.to_path_buf(),
                    reason: "Incomplete line data for parser".to_string(),
                });
            }
        }
    }

    if include_stack.len() != 1 {
        Err(ProcessorError::MismatchedIf {
            path: file_path.to_path_buf(),
        })
    } else {
        Ok(output)
    }
}

/// Processes a single template file.
fn process_file(
    src_path: &Path,
    dest_path: &Path,
    flags: &HashSet<String>,
    used_flags: &mut HashSet<String>,
) -> Result<&'static str> {
    // Returns status string
    let file = File::open(src_path)
        .with_context(|| format!("Failed to open source file: {}", src_path.display()))?;
    let reader = BufReader::new(file);

    let processed_lines = process_content(reader, src_path, flags, used_flags)
        .with_context(|| format!("Failed to process content of: {}", src_path.display()))?;

    if processed_lines.iter().all(|line| line.trim().is_empty()) {
        // Optionally remove the destination file if it exists and is now empty
        // if dest_path.exists() { fs::remove_file(dest_path).ok(); }
        return Ok("skipped");
    }

    // Ensure destination directory exists
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
        // Use writeln! to handle line endings consistently
        writeln!(writer, "{}", line).with_context(|| {
            format!(
                "Failed to write to destination file: {}",
                dest_path.display()
            )
        })?;
    }

    // Ensure buffer is flushed
    writer.flush().with_context(|| {
        format!(
            "Failed to flush writer for destination file: {}",
            dest_path.display()
        )
    })?;

    Ok("written")
}

/// Scans all files in the source directory to find all unique condition flags used.
fn scan_all_conditions(src_dir: &Path) -> Result<HashSet<String>> {
    let mut seen_flags = HashSet::new();
    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let file = File::open(path)
            .with_context(|| format!("Failed to open file for scanning: {}", path.display()))?;
        let reader = BufReader::new(file);

        for line_result in reader.lines() {
            let line = line_result.context("Failed to read line during scan")?;
            // Use the parser to find #if directives and extract condition flags
            if let Ok((_, parser::LineParseResult::If(condition))) = parser::parse_line(&line) {
                seen_flags.extend(condition.mentioned_flags());
            }
            // Ignore lines that don't parse as #if during scan
        }
    }
    Ok(seen_flags)
}

// --- Helper for Unused Flag Suggestions ---
fn find_closest_match<'a>(flag: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .filter(|&&candidate| candidate != flag) // Don't suggest itself
        .min_by_key(|&&candidate| levenshtein(flag, candidate))
        .filter(|&&best_match| {
            let distance = levenshtein(flag, best_match);
            // Simple threshold: distance <= 2 or less than half the length
            let threshold = std::cmp::min(2, flag.len() / 2 + 1);
            distance <= threshold
        })
        .copied()
}

// --- Main Execution ---
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
        let rel_path = src_path.strip_prefix(&args.src_dir)?;
        let dest_path = args.dest_dir.join(rel_path);

        if args.verbose {
            print!("Processing: {} ... ", rel_path.display());
            // Flush stdout to ensure the message appears before potential delay/output
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
                // Should not happen
                if args.verbose {
                    println!("Unknown status: {}", other);
                }
            }
            Err(e) => {
                files_error += 1;
                if args.verbose {
                    // Clear the "Processing..." message before printing error
                    // This works best on terminals supporting cursor movement (most modern ones)
                    // print!("\r\x1b[K"); // Move cursor to beginning, clear line
                    println!(""); // Newline after "Processing..."
                }
                // Use {:?} for anyhow::Error to include context chain
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

        match scan_all_conditions(&args.src_dir) {
            Ok(all_conditions_set) => {
                let all_conditions_vec: Vec<&str> =
                    all_conditions_set.iter().map(String::as_str).collect();

                for &unused_flag in &unused_flags {
                    let mut msg = format!(
                        "  - Flag {} was provided but not used in any evaluated condition.",
                        unused_flag.red()
                    );

                    if !all_conditions_set.contains(unused_flag) {
                        msg.push_str(" It also doesn't appear in any #if condition.");

                        if let Some(suggestion) =
                            find_closest_match(unused_flag, &all_conditions_vec)
                        {
                            msg.push_str(&format!(" Did you mean {}?", suggestion.green()));
                        }
                    } else {
                        msg.push_str(" (It appears in conditions, but they were never 'true').");
                    }

                    println!("{}", msg);
                }

                let mut sorted_used: Vec<_> = used_flags.iter().collect();
                sorted_used.sort_unstable();
                if !sorted_used.is_empty() {
                    println!("\n{}", "Flags effectively used by conditions:".dimmed());
                    print!("  ");
                    for (i, flag) in sorted_used.iter().enumerate() {
                        if i > 0 {
                            print!(" ");
                        }
                        print!("{}", flag.cyan());
                    }
                    println!();
                }

                // Only show "All flags" if it adds value (i.e., different from used or suggests typos)
                let mut sorted_all: Vec<_> = all_conditions_vec.clone();
                sorted_all.sort_unstable();
                if !sorted_all.is_empty() && sorted_all.iter().any(|f| !used_flags.contains(*f)) {
                    println!("\n{}", "All flags found in template conditions:".dimmed());
                    print!("  ");
                    for (i, flag) in sorted_all.iter().enumerate() {
                        if i > 0 {
                            print!(" ");
                        }
                        print!("{}", flag.dimmed());
                    }
                    println!();
                }
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!(
                        "\nWarning: Could not scan for all conditions to provide suggestions: {:?}",
                        e
                    )
                    .yellow()
                );
                for &unused_flag in &unused_flags {
                    println!("  - Unused flag: {}", unused_flag.red());
                }
            }
        }
    }

    // Indicate error to shell if any file processing failed
    if files_error > 0 {
        std::process::exit(1);
    }

    Ok(())
}
