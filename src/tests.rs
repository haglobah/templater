use crate::*;
use std::io::Cursor;

fn make_hashset(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn run_process_content(
    input: &str,
    active_flags: &HashSet<String>,
) -> Result<(Vec<String>, HashSet<String>)> {
    process_content(Cursor::new(input), Path::new("test.txt"), active_flags)
}

// --- parse_condition tests ---

#[test]
fn test_parse_condition_ok() {
    assert_eq!(
        parse_condition("foo").unwrap(),
        Condition::Single("foo".to_string())
    );
    assert_eq!(
        parse_condition(" (and a b) ").unwrap(),
        Condition::And(vec!["a".to_string(), "b".to_string()])
    );
    assert_eq!(
        parse_condition("(or c)").unwrap(),
        Condition::Or(vec!["c".to_string()])
    );
}

#[test]
fn test_parse_condition_err() {
    assert!(parse_condition("(and a").is_err());
    assert!(parse_condition("foo bar").is_err());
    assert!(parse_condition("(and a) extra").is_err());
    assert!(parse_condition("").is_err());
    assert!(parse_condition("()").is_err());
}

// --- Condition::evaluate tests ---

#[test]
fn test_condition_evaluate_single() {
    let flags = make_hashset(&["foo", "bar"]);

    let (result, used) = Condition::Single("foo".to_string()).evaluate(&flags);
    assert!(result);
    assert_eq!(used, make_hashset(&["foo"]));

    let (result, used) = Condition::Single("baz".to_string()).evaluate(&flags);
    assert!(!result);
    assert_eq!(used, make_hashset(&["baz"]));
}

#[test]
fn test_condition_evaluate_and() {
    let flags = make_hashset(&["foo", "bar"]);

    let (result, used) = Condition::And(vec!["foo".into(), "bar".into()]).evaluate(&flags);
    assert!(result);
    assert_eq!(used, make_hashset(&["foo", "bar"]));

    let (result, used) = Condition::And(vec!["foo".into(), "baz".into()]).evaluate(&flags);
    assert!(!result);
    assert_eq!(used, make_hashset(&["foo", "baz"]));

    let (result, used) = Condition::And(vec!["baz".into(), "qux".into()]).evaluate(&flags);
    assert!(!result);
    assert_eq!(used, make_hashset(&["baz", "qux"]));
}

#[test]
fn test_condition_evaluate_or() {
    let flags = make_hashset(&["foo", "bar"]);

    let (result, used) = Condition::Or(vec!["foo".into(), "bar".into()]).evaluate(&flags);
    assert!(result);
    assert_eq!(used, make_hashset(&["foo", "bar"]));

    let (result, used) = Condition::Or(vec!["foo".into(), "baz".into()]).evaluate(&flags);
    assert!(result);
    assert_eq!(used, make_hashset(&["foo", "baz"]));

    let (result, used) = Condition::Or(vec!["baz".into(), "qux".into()]).evaluate(&flags);
    assert!(!result);
    assert_eq!(used, make_hashset(&["baz", "qux"]));
}

// --- process_content tests ---

#[test]
fn test_process_no_directives() {
    let input = "line 1\nline 2";
    let (lines, used) = run_process_content(input, &make_hashset(&["any"])).unwrap();
    assert_eq!(lines, vec!["line 1", "line 2"]);
    assert!(used.is_empty());
}

#[test]
fn test_process_block_if_true() {
    let input = "#if foo\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["foo"])).unwrap();
    assert_eq!(lines, vec!["content"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_block_if_false() {
    let input = "#if foo\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["bar"])).unwrap();
    assert!(lines.is_empty());
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_simple_if_true() {
    let input = "#if foo\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["foo"])).unwrap();
    assert_eq!(lines, vec!["content"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_simple_if_false() {
    let input = "#if foo\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["bar"])).unwrap();
    assert!(lines.is_empty());
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_inline_if_true() {
    let input = "include this #if foo";
    let (lines, used) = run_process_content(input, &make_hashset(&["foo"])).unwrap();
    assert_eq!(lines, vec!["include this"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_inline_if_false() {
    let input = "include this #if foo";
    let (lines, used) = run_process_content(input, &make_hashset(&["bar"])).unwrap();
    assert!(lines.is_empty());
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_inline_if_false_inside_true_block() {
    let input = "#if A\nline1\ncontent #if B\nline3\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["A"])).unwrap();
    assert_eq!(lines, vec!["line1", "line3"]);
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_inline_if_true_inside_true_block() {
    let input = "#if A\nline1\ncontent #if B\nline3\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["A", "B"])).unwrap();
    assert_eq!(lines, vec!["line1", "content", "line3"]);
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_mixed_block_and_inline() {
    let input = "Always #if X\n#if A\nBlock A content\nInline #if B\nMore A\n#endif\nFinal";
    let (lines, used) = run_process_content(input, &make_hashset(&["X", "A"])).unwrap();
    assert_eq!(
        lines,
        vec!["Always", "Block A content", "More A", "Final"]
    );
    assert_eq!(used, make_hashset(&["X", "A", "B"]));
}

#[test]
fn test_process_content_before_after() {
    let input = "before\n#if foo\ncontent\n#endif\nafter";
    let (lines, used) = run_process_content(input, &make_hashset(&["foo"])).unwrap();
    assert_eq!(lines, vec!["before", "content", "after"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_content_before_after_if_false() {
    let input = "before\n#if foo\ncontent\n#endif\nafter";
    let (lines, used) = run_process_content(input, &make_hashset(&["bar"])).unwrap();
    assert_eq!(lines, vec!["before", "after"]);
    assert_eq!(used, make_hashset(&["foo"]));
}

#[test]
fn test_process_nested_if_true_true() {
    let input = "#if A\nouter\n#if B\ninner\n#endif\nouter_end\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["A", "B"])).unwrap();
    assert_eq!(lines, vec!["outer", "inner", "outer_end"]);
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_nested_if_true_false() {
    let input = "#if A\nouter\n#if B\ninner\n#endif\nouter_end\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["A", "C"])).unwrap();
    assert_eq!(lines, vec!["outer", "outer_end"]);
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_nested_if_false() {
    let input = "#if A\nouter\n#if B\ninner\n#endif\nouter_end\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["C", "B"])).unwrap();
    assert!(lines.is_empty());
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_and_true() {
    let input = "#if (and foo bar)\ncontent\n#endif";
    let (lines, used) =
        run_process_content(input, &make_hashset(&["foo", "bar", "baz"])).unwrap();
    assert_eq!(lines, vec!["content"]);
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_and_false() {
    let input = "#if (and foo bar)\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["foo", "baz"])).unwrap();
    assert!(lines.is_empty());
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_or_true() {
    let input = "#if (or foo bar)\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["foo", "baz"])).unwrap();
    assert_eq!(lines, vec!["content"]);
    assert_eq!(used, make_hashset(&["foo", "bar"]));
}

#[test]
fn test_process_or_false() {
    let input = "#if (or foo bar)\ncontent\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["baz", "qux"])).unwrap();
    assert!(lines.is_empty());
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
    let active = make_hashset(&["A", "B", "D", "F"]);
    let (lines, used) = run_process_content(input.trim(), &active).unwrap();
    assert_eq!(
        lines,
        vec![
            "Always here",
            "A block",
            "  D or E block",
            "    F block (inside D or E)",
            "Still A block",
            "Always here too",
        ]
    );
    assert_eq!(used, make_hashset(&["A", "B", "C", "D", "E", "F"]));
}

#[test]
fn test_process_mismatched_endif() {
    let input = "content\n#endif";
    let err = run_process_content(input, &make_hashset(&[])).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("mismatched #endif"), "unexpected error: {msg}");
    assert!(msg.contains("line 2"), "unexpected error: {msg}");
}

#[test]
fn test_process_mismatched_if() {
    let input = "#if A\ncontent";
    let err = run_process_content(input, &make_hashset(&["A"])).unwrap_err();
    assert!(
        err.to_string().contains("mismatched #if"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_process_empty_input() {
    let input = "";
    let (lines, used) = run_process_content(input, &make_hashset(&["A"])).unwrap();
    assert!(lines.is_empty());
    assert!(used.is_empty());
}

#[test]
fn test_process_only_directives() {
    let input = "#if A\n#if B\n#endif\n#endif";
    let (lines, used) = run_process_content(input, &make_hashset(&["A", "B"])).unwrap();
    assert!(lines.is_empty());
    assert_eq!(used, make_hashset(&["A", "B"]));
}

#[test]
fn test_process_invalid_condition_parse() {
    let input = "line1\n#if (and foo\nline2\n#endif";
    let err = run_process_content(input, &make_hashset(&["foo"])).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("(and foo"), "unexpected error: {msg}");
}

// --- find_closest_match tests ---

#[test]
fn test_find_closest_match_found() {
    let candidates = ["apple", "banana", "apricot", "apply"];
    assert_eq!(find_closest_match("appel", &candidates), Some("apple"));
    assert_eq!(find_closest_match("aply", &candidates), Some("apply"));
}

#[test]
fn test_find_closest_match_not_found_distance() {
    let candidates = ["apple", "banana", "apricot"];
    assert_eq!(find_closest_match("orange", &candidates), None);
}

#[test]
fn test_find_closest_match_exact_match() {
    let candidates = ["banana", "apricot"];
    assert_eq!(find_closest_match("apple", &candidates), None);
}

#[test]
fn test_find_closest_match_empty_candidates() {
    let candidates = [];
    assert_eq!(find_closest_match("apple", &candidates), None);
}
