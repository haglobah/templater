use crate::*;

fn identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_whitespace() && c != '(' && c != ')')(input)
}

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

fn parse_single(input: &str) -> IResult<&str, Condition> {
    map(identifier, |flag| Condition::Single(flag.to_string()))(input)
}

fn parse_condition_type(input: &str) -> IResult<&str, Condition> {
    alt((parse_and, parse_or, parse_single))(input)
}

/// Parses a condition string (e.g., extracted by regex).
/// Ensures the entire string is consumed (after trimming).
pub(crate) fn parse_condition_str(input: &str) -> Result<Condition, String> {
    match terminated(parse_condition_type, multispace0)(input.trim()) {
        // Check if the *entire* trimmed input was consumed by the parser
        Ok((remaining, condition)) if remaining.is_empty() => Ok(condition),
        Ok((remaining, _)) => Err(format!(
            "Unexpected trailing characters after condition: '{}'",
            remaining
        )),
        Err(e) => Err(format!("Failed to parse condition structure: {:?}", e)), // Convert nom error to string
    }
}
