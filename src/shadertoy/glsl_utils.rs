// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use std::borrow::Cow;

pub fn replace_in_preprocessor_conditionals(code: &str, find: &str, replace: &str) -> String {
    let re = regex::Regex::new(r"(?m)^(\s*#(?:if|elif)\s+)(.*)$").unwrap();

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
            (State::Outside, '"', _) => {
                output.push(c);
                state = State::InString;
            }
            (State::Outside, '/', Some('/')) => {
                chars.next();
                output.push(' ');
                state = State::InLineComment;
            }
            (State::Outside, '/', Some('*')) => {
                chars.next();
                output.push(' ');
                state = State::InBlockComment;
            }
            (State::Outside, _, _) => {
                output.push(c);
            }
            (State::InString, '\\', _) => {
                output.push(c);
                if let Some(escaped_char) = chars.next() {
                    output.push(escaped_char);
                }
            }
            (State::InString, '"', _) => {
                output.push(c);
                state = State::Outside;
            }
            (State::InString, _, _) => {
                output.push(c);
            }
            (State::InLineComment, '\n', _) => {
                output.push(c);
                state = State::Outside;
            }
            (State::InBlockComment, '*', Some('/')) => {
                chars.next();
                state = State::Outside;
            }
            (State::InLineComment | State::InBlockComment, _, _) => {}
        }
    }

    Cow::Owned(output)
}
