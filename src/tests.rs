// (Keep all your existing code above)

// --- Test Module ---
use crate::*; // Import items from the outer scope (main.rs)
use std::io::Cursor;

// Helper to create a HashSet from string slices
fn make_hashset(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// Helper function to run process_content and check output/used_flags
fn run_process_content(
    input: &str,
    active_flags: &HashSet<String>,
) -> (Result<Vec<String>, ProcessorError>, HashSet<String>) {
    let reader = Cursor::new(input);
    let mut used_flags = HashSet::new();
    let result = process_content(reader, Path::new("test.txt"), active_flags, &mut used_flags);
    (result, used_flags)
}

// --- Parser Tests (`nom`) ---
#[test]
fn test_parser_if_single() {
    assert_eq!(
        parser::parse_line("#if foo"),
        Ok((
            "",
            parser::LineParseResult::If(Condition::Single("foo".to_string()))
        ))
    );
    assert_eq!(
        parser::parse_line("  #if   bar  "), // Whitespace
        Ok((
            "",
            parser::LineParseResult::If(Condition::Single("bar".to_string()))
        ))
    );
}

#[test]
fn test_parser_if_and() {
    assert_eq!(
        parser::parse_line("#if (and foo bar baz)"),
        Ok((
            "",
            parser::LineParseResult::If(Condition::And(vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ]))
        ))
    );
    assert_eq!(
        parser::parse_line(" #if (and  f1   f2 ) "), // Extra whitespace
        Ok((
            "",
            parser::LineParseResult::If(Condition::And(vec!["f1".to_string(), "f2".to_string()]))
        ))
    );
}

#[test]
fn test_parser_if_or() {
    assert_eq!(
        parser::parse_line("#if (or foo bar baz)"),
        Ok((
            "",
            parser::LineParseResult::If(Condition::Or(vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ]))
        ))
    );
    assert_eq!(
        parser::parse_line(" #if (or  f1   f2 ) "), // Extra whitespace
        Ok((
            "",
            parser::LineParseResult::If(Condition::Or(vec!["f1".to_string(), "f2".to_string()]))
        ))
    );
}

#[test]
fn test_parser_endif() {
    assert_eq!(
        parser::parse_line("#endif"),
        Ok(("", parser::LineParseResult::Endif))
    );
    assert_eq!(
        parser::parse_line("  #endif   "), // Whitespace
        Ok(("", parser::LineParseResult::Endif))
    );
    // Nom treats content after endif on same line as *part* of the Endif recognition
    // because we used `recognize`. If we just used `tag("#endif")`, the rest would be leftover.
    assert_eq!(
        parser::parse_line("#endif // comment"),
        Ok(("// comment", parser::LineParseResult::Endif)) // Recognize stops after #endif + whitespace
                                                           // Ok(("", parser::LineParseResult::Endif)) // If using terminated(tag("#endif"), multispace0)
    );
}

#[test]
fn test_parser_content() {
    assert_eq!(
        parser::parse_line("This is a content line"),
        Ok((
            "",
            parser::LineParseResult::Content("This is a content line")
        ))
    );
    assert_eq!(
        parser::parse_line("  Indented content"),
        Ok(("", parser::LineParseResult::Content("  Indented content")))
    );
    assert_eq!(
        parser::parse_line(""), // Empty line
        Ok(("", parser::LineParseResult::Content("")))
    );
}

#[test]
fn test_parser_invalid_if_syntax() {
    // Our specific parser expects '#if condition', otherwise it's content
    // Invalid conditions *within* a recognized #if are handled by process_content returning Err
    assert!(parser::parse_line("#if(").is_err()); // Missing space
    assert!(parser::parse_line("#if (and foo").is_err()); // Missing closing paren
    assert!(parser::parse_line("#if (and").is_err()); // Missing flags and paren
}

// --- Condition Evaluation Tests ---
#[test]
fn test_condition_evaluate_single() {
    let flags = make_hashset(&["foo", "bar"]);
    let mut used = HashSet::new();
    assert!(Condition::Single("foo".to_string()).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo"]));

    used.clear();
    assert!(!Condition::Single("baz".to_string()).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["baz"]));
}

#[test]
fn test_condition_evaluate_and() {
    let flags = make_hashset(&["foo", "bar"]);
    let mut used = HashSet::new();

    // All present
    assert!(Condition::And(vec!["foo".to_string(), "bar".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo", "bar"]));
    used.clear();

    // Some present
    assert!(
        !Condition::And(vec!["foo".to_string(), "baz".to_string()]).evaluate(&flags, &mut used)
    );
    assert_eq!(used, make_hashset(&["foo", "baz"]));
    used.clear();

    // None present
    assert!(
        !Condition::And(vec!["baz".to_string(), "qux".to_string()]).evaluate(&flags, &mut used)
    );
    assert_eq!(used, make_hashset(&["baz", "qux"]));
}

#[test]
fn test_condition_evaluate_or() {
    let flags = make_hashset(&["foo", "bar"]);
    let mut used = HashSet::new();

    // All present (still true)
    assert!(Condition::Or(vec!["foo".to_string(), "bar".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo", "bar"]));
    used.clear();

    // Some present
    assert!(Condition::Or(vec!["foo".to_string(), "baz".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo", "baz"]));
    used.clear();

    // None present
    assert!(!Condition::Or(vec!["baz".to_string(), "qux".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["baz", "qux"]));
}

// --- process_content Tests ---

#[test]
fn test_process_no_directives() {
    let input = "line 1\nline 2";
    let (result, used) = run_process_content(input, &make_hashset(&["any"]));
    assert_eq!(result.unwrap(), vec!["line 1", "line 2"]);
    assert!(used.is_empty());
}

#[test]
fn test_process_simple_if_true() {
    let input = "#if foo\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["foo"]));
    assert_eq!(result.unwrap(), vec!["content"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_simple_if_false() {
    let input = "#if foo\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["bar"]));
    assert!(result.unwrap().is_empty());
    assert_eq!(used, make_hashset(&["foo"])); // foo was still checked
}

#[test]
fn test_process_content_before_after() {
    let input = "before\n#if foo\ncontent\n#endif\nafter";
    let (result, used) = run_process_content(input, &make_hashset(&["foo"]));
    assert_eq!(result.unwrap(), vec!["before", "content", "after"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_content_before_after_if_false() {
    let input = "before\n#if foo\ncontent\n#endif\nafter";
    let (result, used) = run_process_content(input, &make_hashset(&["bar"]));
    assert_eq!(result.unwrap(), vec!["before", "after"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_nested_if_true_true() {
    let input = "#if A\nouter\n#if B\ninner\n#endif\nouter_end\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["A", "B"]));
    assert_eq!(result.unwrap(), vec!["outer", "inner", "outer_end"]);
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_nested_if_true_false() {
    let input = "#if A\nouter\n#if B\ninner\n#endif\nouter_end\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["A", "C"])); // B is false
    assert_eq!(result.unwrap(), vec!["outer", "outer_end"]);
    assert_eq!(used, make_hashset(&["A", "B"])); // B was still evaluated
}

#[test]
fn test_process_nested_if_false() {
    // If outer is false, inner condition B shouldn't even be evaluated
    let input = "#if A\nouter\n#if B\ninner\n#endif\nouter_end\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["C", "B"])); // A is false
    assert!(result.unwrap().is_empty());
    assert_eq!(used, make_hashset(&["A"])); // Only A was evaluated
}

#[test]
fn test_process_and_true() {
    let input = "#if (and foo bar)\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["foo", "bar", "baz"]));
    assert_eq!(result.unwrap(), vec!["content"]);
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_and_false() {
    let input = "#if (and foo bar)\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["foo", "baz"]));
    assert!(result.unwrap().is_empty());
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_or_true() {
    let input = "#if (or foo bar)\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["foo", "baz"]));
    assert_eq!(result.unwrap(), vec!["content"]);
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_or_false() {
    let input = "#if (or foo bar)\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["baz", "qux"]));
    assert!(result.unwrap().is_empty());
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_complex_nesting() {
    let input = r#"
Always here
#if A
A block
#if (and B C)
  B and C block
#endif
#if (or D E)
  D or E block
  #if F
    F block (inside D or E)
  #endif
#endif
Still A block
#endif
Always here too"#;
    let active = make_hashset(&["A", "B", "D", "F"]); // C=false, E=false
    let (result, used) = run_process_content(input.trim(), &active);
    assert_eq!(
        result.unwrap(),
        vec![
            "Always here",
            "A block",
            // B and C block is skipped (C=false)
            "  D or E block",              // D=true
            "    F block (inside D or E)", // F=true
            "Still A block",
            "Always here too",
        ]
    );
    // Check all make_hashset mentioned in evaluated paths
    assert_eq!(used, make_hashset(&["A", "B", "C", "D", "E", "F"]));
}

#[test]
fn test_process_mismatched_endif() {
    let input = "content\n#endif";
    let (result, _) = run_process_content(input, &make_hashset(&[]));
    assert!(matches!(
        result,
        Err(ProcessorError::MismatchedEndif { line_num: 2, .. })
    ));
}

#[test]
fn test_process_mismatched_if() {
    let input = "#if A\ncontent";
    let (result, _) = run_process_content(input, &make_hashset(&["A"]));
    assert!(matches!(result, Err(ProcessorError::MismatchedIf { .. })));
}

#[test]
fn test_process_empty_input() {
    let input = "";
    let (result, used) = run_process_content(input, &make_hashset(&["A"]));
    assert!(result.unwrap().is_empty());
    assert!(used.is_empty());
}

#[test]
fn test_process_only_directives() {
    let input = "#if A\n#if B\n#endif\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["A", "B"]));
    assert!(result.unwrap().is_empty());
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_invalid_condition_parse() {
    let input = "line1\n#if (and foo\nline2\n#endif"; // Malformed condition
    let (result, _used) = run_process_content(input, &make_hashset(&["foo"]));
    // The parser::parse_line should return an error here
    assert!(matches!(
        result,
        Err(ProcessorError::ConditionParse { line_num: 2, .. })
    ));
    // Check that the error message contains the problematic condition string
    if let Err(ProcessorError::ConditionParse { condition, .. }) = result {
        assert!(condition.contains("(and foo"));
    }
}

// --- scan_all_conditions Tests ---
#[test]
fn test_scan_flags() {
    // Use walkdir setup if testing actual file system, otherwise simulate reader
    let input_str = r#"
        #if A
        #if (and B C)
        #endif
        #if (or D A) // A is duplicate
        #endif
        Not a directive
        #if E
        #endif
        "#;
    let src_dir = Path::new("."); // Dummy path, not actually used by reader simulation
    // Simulate reading by parsing lines directly
    let mut seen_flags = HashSet::new();
    for line in input_str.lines() {
        if let Ok((_, parser::LineParseResult::If(condition))) = parser::parse_line(line) {
            seen_flags.extend(condition.mentioned_flags());
        }
    }

    assert_eq!(seen_flags, make_hashset(&["A", "B", "C", "D", "E"]));
}

#[test]
fn test_scan_no_flags() {
    let input_str = "line1\nline2\n#endif // Mismatched ok for scan";
    let mut seen_flags = HashSet::new();
    for line in input_str.lines() {
        if let Ok((_, parser::LineParseResult::If(condition))) = parser::parse_line(line) {
            seen_flags.extend(condition.mentioned_flags());
        }
    }
    assert!(seen_flags.is_empty());
}

// --- find_closest_match Tests ---
#[test]
fn test_find_closest_match_found() {
    let candidates = ["apple", "banana", "apricot", "apply"];
    assert_eq!(find_closest_match("appel", &candidates), Some("apple"));
    assert_eq!(find_closest_match("aply", &candidates), Some("apply"));
}

#[test]
fn test_find_closest_match_not_found_distance() {
    let candidates = ["apple", "banana", "apricot"];
    assert_eq!(find_closest_match("orange", &candidates), None); // Too different
}

#[test]
fn test_find_closest_match_exact_match() {
    let candidates = ["apple", "banana", "apricot"];
    assert_eq!(find_closest_match("apple", &candidates), None); // Exact match is excluded
}

#[test]
fn test_find_closest_match_empty_candidates() {
    let candidates = [];
    assert_eq!(find_closest_match("apple", &candidates), None);
}
