// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use regex::Regex;
use std::collections::HashMap;

use crate::renderer::shader::ShaderError;

use super::{glsl_preprocessor::ShaderError::PreprocessError, glsl_utils::strip_comments};

enum BranchState {
    Searching,
    Active,
    Done,
}

#[derive(Debug, PartialEq)]
enum Token {
    Number(i64),
    Op(String),
    LParen,
    RParen,
}

#[derive(Debug, Clone)]
struct MacroDef {
    params: Option<Vec<String>>,
    body: String,
}

struct GlslPreprocessor {
    defines: HashMap<String, MacroDef>,
    if_stack: Vec<BranchState>,
    line_number: usize,
}

pub fn preprocess(source: &str) -> Result<String, ShaderError> {
    let mut preprocessor = GlslPreprocessor::new();
    preprocessor.run(source)
}

impl GlslPreprocessor {
    fn new() -> Self {
        GlslPreprocessor {
            defines: HashMap::new(),
            if_stack: Vec::new(),
            line_number: 0,
        }
    }

    fn is_active(&self) -> bool {
        self.if_stack
            .last()
            .is_none_or(|state| matches!(state, BranchState::Active))
    }

    fn run(&mut self, source: &str) -> Result<String, ShaderError> {
        self.defines.clear();
        self.if_stack.clear();
        self.line_number = 0;

        let source = source
            .replace("\r\n", "\n")
            .replace("\n\r", "\n")
            .replace("\r", "\n");

        let source = source.replace("\\\n", "");

        let source_no_comments = strip_comments(&source);

        let mut output = String::new();
        let mut active_buffer = String::new();

        for line in source_no_comments.lines() {
            self.line_number += 1;
            let trimmed_line = line.trim();

            if trimmed_line.starts_with('#') {
                if self.is_active() && !active_buffer.is_empty() {
                    let expanded = self.expand_macros(&active_buffer);
                    output.push_str(&expanded);
                    active_buffer.clear();
                }
                if let Some(directive) = get_directive_name(trimmed_line) {
                    match directive {
                        "define" => {
                            if self.is_active() {
                                self.handle_define(trimmed_line);
                            }
                        }
                        "undef" => {
                            if self.is_active() {
                                self.handle_undef(trimmed_line);
                            }
                        }
                        "ifdef" => self.handle_ifdef(trimmed_line),
                        "ifndef" => self.handle_ifndef(trimmed_line),
                        "if" => self.handle_if(trimmed_line),
                        "elif" => self.handle_elif(trimmed_line),
                        "else" => self.handle_else(),
                        "endif" => self.handle_endif(),
                        "error" => {
                            if self.is_active() {
                                return Err(self.handle_error(trimmed_line));
                            }
                        }
                        "pragma" | "extension" | "version" | "line" => {}
                        _ => {
                            return Err(PreprocessError(
                                format!("Unknown directive ({directive})"),
                                self.line_number,
                            ))
                        }
                    }
                }
            } else if self.is_active() {
                active_buffer.push_str(line);
                active_buffer.push('\n');
            }
        }

        if !active_buffer.is_empty() {
            let expanded = self.expand_macros(&active_buffer);
            output.push_str(&expanded);
        }

        Ok(output)
    }

    fn handle_define(&mut self, line: &str) {
        let func_re =
            Regex::new(r"#\s*define\s+([a-zA-Z_][a-zA-Z_0-9]*)\(([^)]*)\)\s*(.*)").unwrap();
        let obj_re = Regex::new(r"#\s*define\s+([a-zA-Z_][a-zA-Z_0-9]*)\s*(.*)").unwrap();

        if let Some(caps) = func_re.captures(line) {
            let name = caps.get(1).unwrap().as_str().to_string();
            let params_str = caps.get(2).unwrap().as_str();
            let params: Vec<String> = if params_str.is_empty() {
                vec![]
            } else {
                params_str
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .collect()
            };
            let body = caps.get(3).unwrap().as_str().trim().to_string();
            self.defines.insert(
                name,
                MacroDef {
                    params: Some(params),
                    body,
                },
            );
        } else if let Some(caps) = obj_re.captures(line) {
            let name = caps.get(1).unwrap().as_str().to_string();
            let body = caps.get(2).unwrap().as_str().trim().to_string();
            self.defines.insert(name, MacroDef { params: None, body });
        }
    }

    fn handle_undef(&mut self, line: &str) {
        let after_hash = line[1..].trim_start();
        let parts: Vec<&str> = after_hash.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "undef" {
            self.defines.remove(parts[1]);
        }
    }

    fn handle_ifdef(&mut self, line: &str) {
        let after_hash = line[1..].trim_start();
        let parts: Vec<&str> = after_hash.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "ifdef" {
            if self.is_active() {
                if self.defines.contains_key(parts[1]) {
                    self.if_stack.push(BranchState::Active);
                } else {
                    self.if_stack.push(BranchState::Searching);
                }
            } else {
                self.if_stack.push(BranchState::Done);
            }
        }
    }

    fn handle_ifndef(&mut self, line: &str) {
        let after_hash = line[1..].trim_start();
        let parts: Vec<&str> = after_hash.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "ifndef" {
            if self.is_active() {
                if !self.defines.contains_key(parts[1]) {
                    self.if_stack.push(BranchState::Active);
                } else {
                    self.if_stack.push(BranchState::Searching);
                }
            } else {
                self.if_stack.push(BranchState::Done);
            }
        }
    }

    fn handle_if(&mut self, line: &str) {
        if !self.is_active() {
            self.if_stack.push(BranchState::Done);
            return;
        }

        let after_hash = line[1..].trim_start();
        let condition_str = after_hash.strip_prefix("if").unwrap_or("").trim();

        if self.evaluate_if_expr(condition_str) {
            self.if_stack.push(BranchState::Active);
        } else {
            self.if_stack.push(BranchState::Searching);
        }
    }

    fn handle_elif(&mut self, line: &str) {
        let condition_is_true = match self.if_stack.last() {
            Some(BranchState::Searching) => {
                let after_hash = line[1..].trim_start();
                let condition_str = after_hash.strip_prefix("elif").unwrap_or("").trim();
                self.evaluate_if_expr(condition_str)
            }
            _ => false,
        };

        if let Some(top) = self.if_stack.last_mut() {
            match top {
                BranchState::Searching if condition_is_true => *top = BranchState::Active,
                BranchState::Active => *top = BranchState::Done,
                _ => {}
            }
        }
    }

    fn handle_else(&mut self) {
        if let Some(top) = self.if_stack.last_mut() {
            match top {
                BranchState::Searching => *top = BranchState::Active,
                BranchState::Active => *top = BranchState::Done,
                BranchState::Done => {}
            }
        }
    }

    fn handle_endif(&mut self) {
        self.if_stack.pop();
    }

    fn handle_error(&self, line: &str) -> ShaderError {
        let after_hash = line[1..].trim_start();
        let error_message = after_hash.strip_prefix("error").unwrap_or("").trim();

        let message = if error_message.is_empty() {
            "Error directive encountered".to_string()
        } else if (error_message.starts_with('"') && error_message.ends_with('"'))
            || (error_message.starts_with('\'') && error_message.ends_with('\''))
        {
            error_message[1..error_message.len() - 1].to_string()
        } else {
            error_message.to_string()
        };

        PreprocessError(message, self.line_number)
    }

    fn evaluate_if_expr(&self, expr: &str) -> bool {
        let defined_re = Regex::new(
            r"defined\s*\(\s*([a-zA-Z_][a-zA-Z_0-9]*)\s*\)|defined\s+([a-zA-Z_][a-zA-Z_0-9]*)",
        )
        .unwrap();
        let replaced_expr = defined_re
            .replace_all(expr, |caps: &regex::Captures| {
                let name = if let Some(m) = caps.get(1) {
                    m.as_str()
                } else {
                    caps.get(2).unwrap().as_str()
                };
                if self.defines.contains_key(name) {
                    "1"
                } else {
                    "0"
                }
            })
            .to_string();

        let expanded_expr = self.expand_macros(&replaced_expr);
        let expr_no_ws = expanded_expr.replace(char::is_whitespace, "");

        let tokens = match self.tokenize(&expr_no_ws) {
            Ok(tokens) => tokens,
            Err(_) => return false,
        };

        if tokens.is_empty() {
            return false;
        }

        let mut index = 0;
        let result = self.parse_expression(&tokens, &mut index);
        index == tokens.len() && result != 0
    }

    fn tokenize(&self, expr: &str) -> Result<Vec<Token>, ()> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '(' => tokens.push(Token::LParen),
                ')' => tokens.push(Token::RParen),
                '0'..='9' => {
                    let mut num_str = String::new();
                    num_str.push(c);

                    if c == '0' && matches!(chars.peek(), Some('x') | Some('X')) {
                        num_str.push(chars.next().unwrap());
                        while let Some(&next_char) = chars.peek() {
                            if next_char.is_ascii_hexdigit() {
                                num_str.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                        if num_str.len() > 2 {
                            let num = i64::from_str_radix(&num_str[2..], 16).map_err(|_| ())?;
                            tokens.push(Token::Number(num));
                        } else {
                            return Err(());
                        }
                    } else if c == '0' && chars.peek().is_some_and(|&ch| ('0'..='7').contains(&ch))
                    {
                        while let Some(&next_char) = chars.peek() {
                            if ('0'..='7').contains(&next_char) {
                                num_str.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                        let num = if num_str.len() > 1 {
                            i64::from_str_radix(&num_str[1..], 8).map_err(|_| ())?
                        } else {
                            0
                        };
                        tokens.push(Token::Number(num));
                    } else {
                        while let Some(&next_char) = chars.peek() {
                            if next_char.is_ascii_digit() {
                                num_str.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                        let num = num_str.parse().map_err(|_| ())?;
                        tokens.push(Token::Number(num));
                    }
                }
                '&' => {
                    if let Some('&') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("&&".to_string()));
                    } else {
                        tokens.push(Token::Op("&".to_string()));
                    }
                }
                '|' => {
                    if let Some('|') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("||".to_string()));
                    } else {
                        tokens.push(Token::Op("|".to_string()));
                    }
                }
                '<' => {
                    if let Some('<') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("<<".to_string()));
                    } else if let Some('=') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("<=".to_string()));
                    } else {
                        tokens.push(Token::Op("<".to_string()));
                    }
                }
                '>' => {
                    if let Some('>') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op(">>".to_string()));
                    } else if let Some('=') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op(">=".to_string()));
                    } else {
                        tokens.push(Token::Op(">".to_string()));
                    }
                }
                '=' => {
                    if let Some('=') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("==".to_string()));
                    } else {
                        return Err(());
                    }
                }
                '!' => {
                    if let Some('=') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("!=".to_string()));
                    } else {
                        tokens.push(Token::Op("!".to_string()));
                    }
                }
                '+' => tokens.push(Token::Op("+".to_string())),
                '-' => tokens.push(Token::Op("-".to_string())),
                '*' => tokens.push(Token::Op("*".to_string())),
                '/' => tokens.push(Token::Op("/".to_string())),
                '%' => tokens.push(Token::Op("%".to_string())),
                '^' => {
                    if let Some('^') = chars.peek() {
                        chars.next();
                        tokens.push(Token::Op("^^".to_string()));
                    } else {
                        tokens.push(Token::Op("^".to_string()));
                    }
                }
                '~' => tokens.push(Token::Op("~".to_string())),
                _ => return Err(()),
            }
        }

        Ok(tokens)
    }

    fn parse_expression(&self, tokens: &[Token], index: &mut usize) -> i64 {
        self.parse_logical_or(tokens, index)
    }

    fn parse_logical_or(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_logical_and(tokens, index);
        while *index < tokens.len() {
            if let Token::Op(op) = &tokens[*index] {
                if op == "||" {
                    *index += 1;
                    let right = self.parse_logical_and(tokens, index);
                    left = if left != 0 || right != 0 { 1 } else { 0 };
                    continue;
                }
            }
            break;
        }
        left
    }

    fn parse_logical_and(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_logical_xor(tokens, index);
        while *index < tokens.len() {
            if let Token::Op(op) = &tokens[*index] {
                if op == "&&" {
                    *index += 1;
                    let right = self.parse_logical_xor(tokens, index);
                    left = if left != 0 && right != 0 { 1 } else { 0 };
                    continue;
                }
            }
            break;
        }
        left
    }

    fn parse_logical_xor(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_bitwise_or(tokens, index);
        while *index < tokens.len() {
            if let Token::Op(op) = &tokens[*index] {
                if op == "^^" {
                    *index += 1;
                    let right = self.parse_bitwise_or(tokens, index);
                    left = if (left != 0) != (right != 0) { 1 } else { 0 };
                    continue;
                }
            }
            break;
        }
        left
    }

    fn parse_bitwise_or(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_bitwise_xor(tokens, index);
        while *index < tokens.len() {
            if let Token::Op(op) = &tokens[*index] {
                if op == "|" {
                    *index += 1;
                    let right = self.parse_bitwise_xor(tokens, index);
                    left |= right;
                    continue;
                }
            }
            break;
        }
        left
    }

    fn parse_bitwise_xor(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_equality(tokens, index);
        while *index < tokens.len() {
            if let Token::Op(op) = &tokens[*index] {
                if op == "^" {
                    *index += 1;
                    let right = self.parse_equality(tokens, index);
                    left ^= right;
                    continue;
                }
            }
            break;
        }
        left
    }

    fn parse_bitwise_and(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_relational(tokens, index);
        while *index < tokens.len() {
            if let Token::Op(op) = &tokens[*index] {
                if op == "&" {
                    *index += 1;
                    let right = self.parse_relational(tokens, index);
                    left &= right;
                    continue;
                }
            }
            break;
        }
        left
    }

    fn parse_equality(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_bitwise_and(tokens, index);
        while *index < tokens.len() {
            match &tokens[*index] {
                Token::Op(op) if op == "==" => {
                    *index += 1;
                    let right = self.parse_bitwise_and(tokens, index);
                    left = if left == right { 1 } else { 0 };
                }
                Token::Op(op) if op == "!=" => {
                    *index += 1;
                    let right = self.parse_bitwise_and(tokens, index);
                    left = if left != right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_relational(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_shift(tokens, index);
        while *index < tokens.len() {
            match &tokens[*index] {
                Token::Op(op) if op == "<" => {
                    *index += 1;
                    let right = self.parse_shift(tokens, index);
                    left = if left < right { 1 } else { 0 };
                }
                Token::Op(op) if op == "<=" => {
                    *index += 1;
                    let right = self.parse_shift(tokens, index);
                    left = if left <= right { 1 } else { 0 };
                }
                Token::Op(op) if op == ">" => {
                    *index += 1;
                    let right = self.parse_shift(tokens, index);
                    left = if left > right { 1 } else { 0 };
                }
                Token::Op(op) if op == ">=" => {
                    *index += 1;
                    let right = self.parse_shift(tokens, index);
                    left = if left >= right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_shift(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_additive(tokens, index);
        while *index < tokens.len() {
            match &tokens[*index] {
                Token::Op(op) if op == "<<" => {
                    *index += 1;
                    let right = self.parse_additive(tokens, index);
                    left = left.wrapping_shl(right as u32);
                }
                Token::Op(op) if op == ">>" => {
                    *index += 1;
                    let right = self.parse_additive(tokens, index);
                    left = left.wrapping_shr(right as u32);
                }
                _ => break,
            }
        }
        left
    }

    fn parse_additive(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_multiplicative(tokens, index);
        while *index < tokens.len() {
            match &tokens[*index] {
                Token::Op(op) if op == "+" => {
                    *index += 1;
                    let right = self.parse_multiplicative(tokens, index);
                    left = left.wrapping_add(right);
                }
                Token::Op(op) if op == "-" => {
                    *index += 1;
                    let right = self.parse_multiplicative(tokens, index);
                    left = left.wrapping_sub(right);
                }
                _ => break,
            }
        }
        left
    }

    fn parse_multiplicative(&self, tokens: &[Token], index: &mut usize) -> i64 {
        let mut left = self.parse_unary(tokens, index);
        while *index < tokens.len() {
            match &tokens[*index] {
                Token::Op(op) if op == "*" => {
                    *index += 1;
                    let right = self.parse_unary(tokens, index);
                    left = left.wrapping_mul(right);
                }
                Token::Op(op) if op == "/" => {
                    *index += 1;
                    let right = self.parse_unary(tokens, index);
                    if right == 0 {
                        left = 0; // Division by zero
                    } else {
                        left = left.wrapping_div(right);
                    }
                }
                Token::Op(op) if op == "%" => {
                    *index += 1;
                    let right = self.parse_unary(tokens, index);
                    if right == 0 {
                        left = 0; // Division by zero
                    } else {
                        left = left.wrapping_rem(right);
                    }
                }
                _ => break,
            }
        }
        left
    }

    fn parse_unary(&self, tokens: &[Token], index: &mut usize) -> i64 {
        if *index >= tokens.len() {
            return 0;
        }
        match &tokens[*index] {
            Token::Op(op) if op == "~" => {
                *index += 1;
                !self.parse_unary(tokens, index)
            }
            Token::Op(op) if op == "!" => {
                *index += 1;
                let val = self.parse_unary(tokens, index);
                if val == 0 {
                    1
                } else {
                    0
                }
            }
            Token::Op(op) if op == "-" => {
                *index += 1;
                -self.parse_unary(tokens, index)
            }
            Token::Op(op) if op == "+" => {
                *index += 1;
                self.parse_unary(tokens, index)
            }
            _ => self.parse_primary(tokens, index),
        }
    }

    fn parse_primary(&self, tokens: &[Token], index: &mut usize) -> i64 {
        if *index >= tokens.len() {
            return 0;
        }
        match &tokens[*index] {
            Token::Number(n) => {
                *index += 1;
                *n
            }
            Token::LParen => {
                *index += 1;
                let expr = self.parse_expression(tokens, index);
                if *index < tokens.len() && matches!(tokens[*index], Token::RParen) {
                    *index += 1;
                }
                expr
            }
            _ => {
                *index += 1;
                0
            }
        }
    }

    fn expand_macros(&self, line: &str) -> String {
        fn is_identifier_character(c: u8) -> bool {
            c.is_ascii_alphanumeric() || c == b'_'
        }

        let mut current_line = line.to_string();
        loop {
            let mut expanded_in_pass = false;
            let mut earliest_expansion: Option<(usize, usize, String)> = None;

            for (name, def) in &self.defines {
                for (start_index, _) in current_line.match_indices(name) {
                    if earliest_expansion.is_some()
                        && start_index >= earliest_expansion.as_ref().unwrap().0
                    {
                        continue;
                    }
                    let end_index = start_index + name.len();

                    let is_start_boundary = start_index == 0
                        || !is_identifier_character(current_line.as_bytes()[start_index - 1]);
                    let is_end_boundary = end_index == current_line.len()
                        || !is_identifier_character(current_line.as_bytes()[end_index]);

                    if is_start_boundary && is_end_boundary {
                        if let Some(params) = &def.params {
                            if let Some((args_end, args)) =
                                self.parse_macro_args(&current_line, end_index, params.len())
                            {
                                let expanded = self.replace_params(&def.body, params, &args);
                                earliest_expansion = Some((start_index, args_end, expanded));
                            }
                        } else {
                            earliest_expansion = Some((start_index, end_index, def.body.clone()));
                        }
                    }
                }
            }

            if let Some((start, end, replacement)) = earliest_expansion {
                current_line.replace_range(start..end, &replacement);
                expanded_in_pass = true;
            }

            if !expanded_in_pass {
                break;
            }
        }
        current_line
    }

    fn parse_macro_args(
        &self,
        line: &str,
        start_offset: usize,
        arg_count: usize,
    ) -> Option<(usize, Vec<String>)> {
        let mut chars = line[start_offset..].char_indices().peekable();

        while let Some((_, c)) = chars.peek() {
            if !c.is_whitespace() {
                break;
            }
            chars.next();
        }

        if chars.next()?.1 != '(' {
            return None;
        }

        let mut args = Vec::new();
        let mut current_arg = String::new();
        let mut paren_level = 1;
        let mut end_offset = 0;

        if arg_count == 0 {
            let mut final_char_i = 0;
            for (i, c) in chars {
                final_char_i = i;
                if c == ')' {
                    paren_level -= 1;
                    break;
                }
                if !c.is_whitespace() {
                    return None;
                }
            }
            if paren_level == 0 {
                return Some((start_offset + final_char_i + 1, args));
            } else {
                return None;
            }
        }

        for (i, c) in chars {
            end_offset = start_offset + i + 1;
            match c {
                ')' => {
                    paren_level -= 1;
                    if paren_level == 0 {
                        args.push(current_arg.trim().to_string());
                        break;
                    }
                }
                '(' => paren_level += 1,
                ',' => {
                    if paren_level == 1 {
                        args.push(current_arg.trim().to_string());
                        current_arg.clear();
                        continue;
                    }
                }
                _ => {}
            }
            current_arg.push(c);
        }

        if paren_level == 0 && args.len() == arg_count {
            Some((end_offset, args))
        } else {
            None
        }
    }

    fn replace_params(&self, body: &str, params: &[String], args: &[String]) -> String {
        if params.is_empty() {
            return body.to_string();
        }

        let mut sorted_params = params.to_vec();
        sorted_params.sort_by_key(|b| std::cmp::Reverse(b.len()));

        let pattern_parts: Vec<String> = sorted_params
            .iter()
            .map(|p| format!(r"\b{}\b", regex::escape(p)))
            .collect();
        let pattern = pattern_parts.join("|");

        if let Ok(re) = Regex::new(&pattern) {
            re.replace_all(body, |caps: &regex::Captures| {
                let matched = caps.get(0).unwrap().as_str();
                params
                    .iter()
                    .position(|p| p == matched)
                    .map(|idx| args[idx].clone())
                    .unwrap_or_else(|| matched.to_string())
            })
            .to_string()
        } else {
            body.to_string()
        }
    }
}

fn get_directive_name(line: &str) -> Option<&str> {
    let after_hash = line[1..].trim_start();
    after_hash.split_whitespace().next()
}
