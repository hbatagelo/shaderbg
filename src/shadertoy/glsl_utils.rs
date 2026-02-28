// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! GLSL preprocessing utility helpers.
//!
//! Provides low-level string and parsing utilities shared by the
//! ShaderToy GLSL conversion pipeline.

use std::borrow::Cow;

/// Replaces occurrences of a word within `#if` and `#elif` conditions.
pub fn replace_in_preprocessor_conditionals(code: &str, find: &str, replace: &str) -> String {
    let re = regex::Regex::new(r"(?m)^(\s*#(?:if|elif)\s+)(.*)$").unwrap();

    // Escape the find string for use in regex and add word boundaries
    let escaped_find = regex::escape(find);
    let pattern = format!(r"\b{}\b", escaped_find);
    let find_re = regex::Regex::new(&pattern).unwrap();

    re.replace_all(code, |caps: &regex::Captures| {
        let prefix = caps.get(1).unwrap().as_str();
        let condition = caps.get(2).unwrap().as_str();
        let new_condition = find_re.replace_all(condition, replace);
        format!("{}{}", prefix, new_condition)
    })
    .to_string()
}

/// Strips all GLSL comments (`//` and `/* ... */`) from a source string.
/// Per the GLSL spec, each comment is replaced by a single space.
pub fn strip_comments(source: &str) -> Cow<'_, str> {
    if !source.contains("//") && !source.contains("/*") {
        return Cow::Borrowed(source);
    }

    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    #[derive(Clone, Copy)]
    enum State {
        Outside,
        InString,
        InLineComment,
        InBlockComment,
    }

    let mut state = State::Outside;

    while let Some(c) = chars.next() {
        match (state, c, chars.peek()) {
            // Start of a string literal
            (State::Outside, '"', _) => {
                output.push(c);
                state = State::InString;
            }
            // Start of line comment
            (State::Outside, '/', Some('/')) => {
                chars.next();
                output.push(' ');
                state = State::InLineComment;
            }
            // Start of block comment
            (State::Outside, '/', Some('*')) => {
                chars.next();
                output.push(' ');
                state = State::InBlockComment;
            }
            // Any other character when outside comments/strings
            (State::Outside, _, _) => {
                output.push(c);
            }
            // Handle escape sequences in strings
            (State::InString, '\\', _) => {
                output.push(c);
                if let Some(escaped_char) = chars.next() {
                    output.push(escaped_char);
                }
            }
            // End of string literal
            (State::InString, '"', _) => {
                output.push(c);
                state = State::Outside;
            }
            // Any other character inside the string
            (State::InString, _, _) => {
                output.push(c);
            }
            // End of line comment
            (State::InLineComment, '\n', _) => {
                output.push(c); // Preserve the newline
                state = State::Outside;
            }
            // End of block comment
            (State::InBlockComment, '*', Some('/')) => {
                chars.next(); // Consume the '/'
                state = State::Outside;
            }
            // Skip all other characters when inside comments
            (State::InLineComment | State::InBlockComment, _, _) => {}
        }
    }

    Cow::Owned(output)
}
