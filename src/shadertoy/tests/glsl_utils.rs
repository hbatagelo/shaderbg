use pretty_assertions::assert_eq;
use std::borrow::Cow;

use super::super::glsl_utils::strip_comments;

#[test]
fn test_strip_no_comments() {
    let source = "void main() {\n  float x = 10.0 / 2.0;\n}";
    let expected = source;
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_line_comment() {
    let source = "vec3 color; // My output color";
    let expected = "vec3 color;  "; // Replaced with a space
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_line_that_is_only_a_comment() {
    let source = "// A full line comment";
    let expected = " ";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_block_comment_single_line() {
    let source = "float x = 1.0; /* a value */ float y = 2.0;";
    let expected = "float x = 1.0;   float y = 2.0;";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_block_comment_multi_line() {
    let source = "void main() {\n  /* This is a\n   * multi-line comment.\n   */\n  int i = 0;\n}";
    let expected = "void main() {\n   \n  int i = 0;\n}";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_mixed_comments() {
    let source = "/* Block */\n// Line\nint x = 0;";
    let expected = " \n \nint x = 0;";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_consecutive_comments() {
    let source = "int a; /* one */// two";
    let expected = "int a;   ";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_unterminated_block_comment() {
    let source = "vec3 v; /* This comment never ends...";
    let expected = "vec3 v;  ";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_strip_empty_string() {
    let source = "";
    let expected = "";
    assert_eq!(strip_comments(source), expected);
}

#[test]
fn test_comments_in_strings_preserved() {
    let source = r#"#error "File /*not*/ found // this is NOT a comment""#;
    let result = strip_comments(source);
    assert_eq!(result, source);
}

#[test]
fn test_escaped_quotes_in_strings() {
    let source = r#"#error "She said \"Hello // world\"""#;
    let result = strip_comments(source);
    assert_eq!(result, source);
}

#[test]
fn test_mixed_strings_and_comments() {
    let source = r#"#error "Debug mode" // Enable debug
float x = 1.0; /* block comment */"#;
    let expected = "#error \"Debug mode\"  \nfloat x = 1.0;  ";
    let result = strip_comments(source);
    assert_eq!(result, expected);
}

#[test]
fn test_no_comments_returns_borrowed() {
    let source = "vec3 color = vec3(1.0);";
    let result = strip_comments(source);
    assert!(matches!(result, Cow::Borrowed(_)));
}
