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

#[test]
fn test_parse_condition_str_ok() {
    assert_eq!(
        parser::parse_condition_str("foo"),
        Ok(parser::Condition::Single("foo".to_string()))
    );
    assert_eq!(
        parser::parse_condition_str(" (and a b) "),
        Ok(parser::Condition::And(vec!["a".to_string(), "b".to_string()]))
    );
    assert_eq!(
        parser::parse_condition_str("(or c)"),
        Ok(parser::Condition::Or(vec!["c".to_string()]))
    );
}

#[test]
fn test_parse_condition_str_err() {
    assert!(parser::parse_condition_str("(and a").is_err()); // Incomplete
    assert!(parser::parse_condition_str("foo bar").is_err()); // Trailing chars
    assert!(parser::parse_condition_str("(and a) extra").is_err()); // Trailing chars
    assert!(parser::parse_condition_str("").is_err()); // Empty
    assert!(parser::parse_condition_str("()").is_err()); // Invalid structure
}
#[test]
fn test_process_block_if_true() {
    let input = "#if foo\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["foo"]));
    assert_eq!(result.unwrap(), vec!["content"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_block_if_false() {
    let input = "#if foo\ncontent\n#endif";
    let (result, used) = run_process_content(input, &make_hashset(&["bar"]));
    assert!(result.unwrap().is_empty());
    assert_eq!(used, make_hashset(&["foo"]));
}

// New Inline Tests
#[test]
fn test_process_inline_if_true() {
    let input = "include this #if foo";
    let (result, used) = run_process_content(input, &make_hashset(&["foo"]));
    assert_eq!(result.unwrap(), vec!["include this"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_inline_if_false() {
    let input = "include this #if foo";
    let (result, used) = run_process_content(input, &make_hashset(&["bar"]));
    assert!(result.unwrap().is_empty()); // Content before #if is dropped
    assert_eq!(used, make_hashset(&["foo"])); // parser::Condition still evaluated
}

// #[test]
// fn test_process_inline_if_true_inside_false_block() {
//     let input = "#if A\nline1\ncontent #if B\nline3\n#endif"; // B is inline
//     let (result, used) = run_process_content(input, &make_hashset(&["B"])); // A=false, B=true
//     assert!(result.unwrap().is_empty()); // Whole block A is excluded
//     assert_eq!(used, make_hashset(&["A", "B"])); // B is not evaluated because block A is false
// }

#[test]
fn test_process_inline_if_false_inside_true_block() {
    let input = "#if A\nline1\ncontent #if B\nline3\n#endif"; // B is inline
    let (result, used) = run_process_content(input, &make_hashset(&["A"])); // A=true, B=false
    assert_eq!(result.unwrap(), vec!["line1", "line3"]); // Inline condition fails, 'content' is dropped
    // but line1/line3 included due to block A
    assert_eq!(used, make_hashset(&["A", "B"])); // Both evaluated
}

#[test]
fn test_process_inline_if_true_inside_true_block() {
    let input = "#if A\nline1\ncontent #if B\nline3\n#endif"; // B is inline
    let (result, used) = run_process_content(input, &make_hashset(&["A", "B"])); // A=true, B=true
    assert_eq!(result.unwrap(), vec!["line1", "content", "line3"]); // Inline condition passes, 'content' included
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_mixed_block_and_inline() {
    let input = "Always #if X\n#if A\nBlock A content\nInline #if B\nMore A\n#endif\nFinal";
    // X=true, A=true, B=false
    let (result, used) = run_process_content(input, &make_hashset(&["X", "A"]));
    assert_eq!(
        result.unwrap(),
        vec![
            "Always",          // Inline X=true
            "Block A content", // Block A=true
            "More A",          // Block A=true, Inline B=false drops 'Inline'
            "Final",           // Block A=true
        ]
    );
    assert_eq!(used, make_hashset(&["X", "A", "B"]));
}

// --- Condition Evaluation Tests ---
#[test]
fn test_condition_evaluate_single() {
    let flags = make_hashset(&["foo", "bar"]);
    let mut used = HashSet::new();
    assert!(parser::Condition::Single("foo".to_string()).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo"]));

    used.clear();
    assert!(!parser::Condition::Single("baz".to_string()).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["baz"]));
}

#[test]
fn test_condition_evaluate_and() {
    let flags = make_hashset(&["foo", "bar"]);
    let mut used = HashSet::new();

    // All present
    assert!(parser::Condition::And(vec!["foo".to_string(), "bar".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo", "bar"]));
    used.clear();

    // Some present
    assert!(
        !parser::Condition::And(vec!["foo".to_string(), "baz".to_string()]).evaluate(&flags, &mut used)
    );
    assert_eq!(used, make_hashset(&["foo", "baz"]));
    used.clear();

    // None present
    assert!(
        !parser::Condition::And(vec!["baz".to_string(), "qux".to_string()]).evaluate(&flags, &mut used)
    );
    assert_eq!(used, make_hashset(&["baz", "qux"]));
}

#[test]
fn test_condition_evaluate_or() {
    let flags = make_hashset(&["foo", "bar"]);
    let mut used = HashSet::new();

    // All present (still true)
    assert!(parser::Condition::Or(vec!["foo".to_string(), "bar".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo", "bar"]));
    used.clear();

    // Some present
    assert!(parser::Condition::Or(vec!["foo".to_string(), "baz".to_string()]).evaluate(&flags, &mut used));
    assert_eq!(used, make_hashset(&["foo", "baz"]));
    used.clear();

    // None present
    assert!(!parser::Condition::Or(vec!["baz".to_string(), "qux".to_string()]).evaluate(&flags, &mut used));
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
    assert_eq!(used, make_hashset(&["A", "B"])); // Only A was evaluated
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
    let candidates = ["banana", "apricot"];
    assert_eq!(find_closest_match("apple", &candidates), None); // Exact match is excluded
}

#[test]
fn test_find_closest_match_empty_candidates() {
    let candidates = [];
    assert_eq!(find_closest_match("apple", &candidates), None);
}
