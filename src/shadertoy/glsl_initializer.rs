// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

use num_traits::Saturating;
use std::collections::HashMap;

use crate::renderer::shader::ShaderError;

use super::{glsl_depth_tracker::GlslDepthTracker, glsl_preprocessor};

struct StructMember {
    type_name: String,
    _name: String,
    array_specifier: Option<String>,
}

struct StructDefinition {
    _name: String,
    members: Vec<StructMember>,
}

struct GlslInitializer<'a> {
    source_str: &'a str,
    source_bytes: &'a [u8],
    struct_defs: HashMap<String, StructDefinition>,
}

pub fn initialize_uninitialized_variables(source: &str) -> Result<String, ShaderError> {
    let mut source = glsl_preprocessor::preprocess(source)?;

    let modifications = GlslInitializer::new(&source).modifications();
    for (start, end, replacement) in modifications.into_iter().rev() {
        source.replace_range(start..end, &replacement);
    }

    Ok(source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

impl<'a> GlslInitializer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source_str: source,
            source_bytes: source.as_bytes(),
            struct_defs: HashMap::new(),
        }
    }

    fn modifications(&mut self) -> Vec<(usize, usize, String)> {
        #[derive(PartialEq, Eq)]
        enum ParseState {
            Normal,
            InString,
        }

        #[rustfmt::skip]
        const BUILT_IN_TYPES: [&str; 28] = [
            "float", "int", "uint", "bool",
            "vec2",   "vec3",   "vec4",
            "mat2",   "mat3",   "mat4",
            "mat2x2", "mat2x3", "mat2x4",
            "mat3x2", "mat3x3", "mat3x4",
            "mat4x2", "mat4x3", "mat4x4",
            "ivec2",  "ivec3",  "ivec4",
            "uvec2",  "uvec3",  "uvec4",
            "bvec2",  "bvec3",  "bvec4",
        ];

        const SKIP_QUALIFIERS: [&str; 5] = ["const", "uniform", "in", "out", "varying"];

        let mut state = ParseState::Normal;
        let mut depth_tracker = GlslDepthTracker::default();
        let mut i = 0;
        let mut expect_for_paren = false;
        let mut modifications = Vec::new();
        let length = self.source_bytes.len();

        self.struct_defs.clear();

        while i < length {
            match state {
                ParseState::Normal => {
                    match self.source_bytes[i] {
                        b'"' => {
                            state = ParseState::InString;
                            i += 1;
                            continue;
                        }
                        b'#' => {
                            i = self.skip_preprocessor_directive(i);
                            continue;
                        }
                        byte => {
                            depth_tracker.update_brackets(byte);
                            depth_tracker.update_for_loop(byte);
                        }
                    }

                    if depth_tracker.in_parentheses() && expect_for_paren {
                        depth_tracker.start_for_loop_tracking();
                        expect_for_paren = false;
                    }

                    if self.is_identifier_start(self.source_bytes[i]) {
                        let (token, next_i) = self.read_identifier(i);
                        i = next_i;

                        if token == "for" {
                            expect_for_paren = true;
                            continue;
                        }

                        if token == "struct" {
                            let (_, end_pos) = self.parse_struct_definition(i);
                            i = end_pos;
                            continue;
                        }

                        if SKIP_QUALIFIERS.contains(&token) && depth_tracker.at_global_scope() {
                            i = self.skip_to_declaration_end(i);
                            continue;
                        }

                        if BUILT_IN_TYPES.contains(&token) || self.struct_defs.contains_key(token) {
                            if let Some((modification, next_i)) = self.process_type_declaration(
                                token,
                                i,
                                &mut depth_tracker,
                                &self.struct_defs,
                            ) {
                                modifications.push(modification);
                                i = next_i;
                                continue;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }
                ParseState::InString => {
                    if self.source_bytes[i] == b'"' {
                        state = ParseState::Normal;
                    } else if self.source_bytes[i] == b'\\' && i + 1 < length {
                        i += 1; // Skip escaped character
                    }
                    i += 1;
                }
            }
        }

        modifications
    }

    fn is_identifier_start(&self, byte: u8) -> bool {
        byte.is_ascii_alphabetic() || byte == b'_'
    }

    fn read_identifier(&self, start: usize) -> (&str, usize) {
        let mut i = start + 1;
        while i < self.source_bytes.len()
            && (self.source_bytes[i].is_ascii_alphanumeric() || self.source_bytes[i] == b'_')
        {
            i += 1;
        }
        (&self.source_str[start..i], i)
    }

    fn skip_preprocessor_directive(&self, mut i: usize) -> usize {
        i += 1;
        while i < self.source_bytes.len() && self.source_bytes[i] != b'\n' {
            i += 1;
        }
        i
    }

    fn skip_to_declaration_end(&self, mut i: usize) -> usize {
        let mut depth = GlslDepthTracker::default();
        let mut in_string = false;

        while i < self.source_bytes.len() {
            let byte = self.source_bytes[i];
            if in_string {
                if byte == b'"' {
                    in_string = false;
                } else if byte == b'\\' && i + 1 < self.source_bytes.len() {
                    i += 1; // Skip escape next
                }
            } else {
                depth.update_brackets(byte);
                match byte {
                    b'"' => in_string = true,
                    b';' if depth.at_global_scope() => {
                        i += 1; // Skip ';'
                        break;
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        i
    }

    fn parse_struct_definition(&mut self, mut i: usize) -> (Option<String>, usize) {
        while i < self.source_bytes.len() && self.source_bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        let struct_name = if self.is_identifier_start(self.source_bytes[i]) {
            let (name, next_i) = self.read_identifier(i);
            i = next_i;
            Some(name.to_string())
        } else {
            None
        };

        while i < self.source_bytes.len() && self.source_bytes[i] != b'{' {
            i += 1;
        }
        if i >= self.source_bytes.len() {
            return (struct_name, i);
        }

        i += 1; // Skip '{'
        let start_brace = i;
        let mut depth = 1;
        while i < self.source_bytes.len() && depth > 0 {
            match self.source_bytes[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }

        if depth == 0 {
            let end_brace = i - 1;
            if let Some(name) = &struct_name {
                let body = &self.source_str[start_brace..end_brace];
                let members = self.parse_struct_body(body);
                self.struct_defs.insert(
                    name.clone(),
                    StructDefinition {
                        _name: name.clone(),
                        members,
                    },
                );
            }
        }

        (struct_name, i)
    }

    fn parse_struct_body(&self, body: &str) -> Vec<StructMember> {
        let mut members = Vec::new();
        let decls = body.split(';').filter(|s| !s.trim().is_empty());

        for decl in decls {
            let parts: Vec<&str> = decl.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let type_name = parts[0].to_string();
            for var_part in parts[1..].join("").split(',') {
                let var_part = var_part.trim();
                if var_part.is_empty() {
                    continue;
                }

                let (name, array_spec) = if let Some(pos) = var_part.find('[') {
                    (
                        var_part[..pos].to_string(),
                        Some(var_part[pos..].to_string()),
                    )
                } else {
                    (var_part.to_string(), None)
                };

                members.push(StructMember {
                    type_name: type_name.clone(),
                    _name: name,
                    array_specifier: array_spec,
                });
            }
        }
        members
    }

    fn process_type_declaration(
        &self,
        type_name: &str,
        start_pos: usize,
        depth: &mut GlslDepthTracker,
        struct_defs: &HashMap<String, StructDefinition>,
    ) -> Option<((usize, usize, String), usize)> {
        if type_name == "void" {
            return None;
        }

        if (depth.in_parentheses() || depth.in_square_brackets())
            && !depth.in_for_loop_initialization()
        {
            return None;
        }

        let mut i = start_pos;
        while i < self.source_bytes.len() && self.source_bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        if depth.at_global_scope() && self.is_function_definition(i) {
            return None;
        }

        let type_end = i;
        let start_depth = *depth;
        let semicolon_pos = self.find_declaration_end(i, start_depth)?;
        let decl_str = &self.source_str[type_end..semicolon_pos];

        let new_decl_str = self.process_variable_declarations(decl_str, type_name, struct_defs);

        if !new_decl_str.is_empty() && new_decl_str != decl_str {
            Some(((type_end, semicolon_pos, new_decl_str), semicolon_pos + 1))
        } else {
            None
        }
    }

    fn is_function_definition(&self, mut i: usize) -> bool {
        if i >= self.source_bytes.len() || !self.is_identifier_start(self.source_bytes[i]) {
            return false;
        }
        while i < self.source_bytes.len()
            && (self.source_bytes[i].is_ascii_alphanumeric() || self.source_bytes[i] == b'_')
        {
            i += 1;
        }
        while i < self.source_bytes.len() && self.source_bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        if i >= self.source_bytes.len() || self.source_bytes[i] != b'(' {
            return false;
        }
        i += 1;

        let mut depth = 1;
        while i < self.source_bytes.len() && depth > 0 {
            match self.source_bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            i += 1;
        }

        while i < self.source_bytes.len() && self.source_bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        i < self.source_bytes.len() && matches!(self.source_bytes[i], b';' | b'{')
    }

    fn find_declaration_end(&self, mut i: usize, start_depth: GlslDepthTracker) -> Option<usize> {
        let mut local_depth = start_depth;
        while i < self.source_bytes.len() {
            let byte = self.source_bytes[i];
            if byte == b'"' {
                break;
            }
            local_depth.update_brackets(byte);
            local_depth.update_for_loop(byte);
            if byte == b';' && local_depth == start_depth {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    fn process_variable_declarations(
        &self,
        decl_str: &str,
        type_name: &str,
        struct_defs: &HashMap<String, StructDefinition>,
    ) -> String {
        self.split_by_commas(decl_str)
            .iter()
            .map(|part| {
                let trimmed = part.trim();
                if trimmed.is_empty() || trimmed.contains('=') || trimmed.contains('(') {
                    return trimmed.to_string();
                }

                let (full_variable_decl, array_spec_part) = if trimmed.starts_with('[') {
                    let end_spec_idx = trimmed.rfind(']').map(|i| i + 1).unwrap_or(0);
                    let spec = &trimmed[..end_spec_idx];
                    (trimmed.to_string(), Some(spec.to_string()))
                } else if let Some(start_spec_idx) = trimmed.find('[') {
                    (
                        trimmed.to_string(),
                        Some(trimmed[start_spec_idx..].to_string()),
                    )
                } else {
                    (trimmed.to_string(), None)
                };

                let base_type = type_name.split('[').next().unwrap_or("").trim();
                let type_array_spec = type_name.find('[').map(|i| &type_name[i..]);

                let final_array_spec = match (type_array_spec, array_spec_part) {
                    (Some(s1), Some(s2)) => Some(format!("{}{}", s1, s2)),
                    (Some(s), None) => Some(s.to_string()),
                    (None, Some(s)) => Some(s.to_string()),
                    (None, None) => None,
                };

                if let Some(default_value) =
                    default_value(base_type, final_array_spec.as_deref(), struct_defs)
                {
                    format!("{} = {}", full_variable_decl, default_value)
                } else {
                    full_variable_decl
                }
            })
            .collect::<Vec<String>>()
            .join(", ")
    }

    fn split_by_commas(&self, s: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut depth = 0;
        let mut start = 0;

        for (i, byte) in s.bytes().enumerate() {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    parts.push(s[start..i].to_string());
                    start = i + 1;
                }
                _ => {}
            }
        }
        parts.push(s[start..].to_string());
        parts
    }
}

fn default_value(
    type_str: &str,
    array_spec: Option<&str>,
    struct_defs: &HashMap<String, StructDefinition>,
) -> Option<String> {
    if let Some(spec) = array_spec {
        let re = regex::Regex::new(r"\[\s*(\d+)\s*\]").unwrap();
        let dims: Vec<usize> = re
            .captures_iter(spec)
            .filter_map(|cap| cap.get(1).and_then(|m| m.as_str().parse().ok()))
            .collect();

        if dims.is_empty() || dims.contains(&0) {
            return None;
        }
        generate_array_initializer_recursive(type_str, &dims, struct_defs)
    } else {
        scalar_or_struct_default(type_str, struct_defs)
    }
}

fn generate_array_initializer_recursive(
    base_type: &str,
    dims: &[usize],
    struct_defs: &HashMap<String, StructDefinition>,
) -> Option<String> {
    if dims.is_empty() {
        return scalar_or_struct_default(base_type, struct_defs);
    }

    let last_dim = *dims.last().unwrap();
    let inner_dims = &dims[..dims.len() - 1];

    let element_initializer =
        generate_array_initializer_recursive(base_type, inner_dims, struct_defs)?;

    let initializers: Vec<_> = (0..last_dim).map(|_| element_initializer.clone()).collect();

    let constructor_type = format!(
        "{}{}",
        base_type,
        inner_dims
            .iter()
            .map(|d| format!("[{}]", d))
            .collect::<String>()
    );

    Some(format!(
        "{}[{}]({})",
        constructor_type,
        last_dim,
        initializers.join(", ")
    ))
}

fn scalar_or_struct_default(
    type_str: &str,
    struct_defs: &HashMap<String, StructDefinition>,
) -> Option<String> {
    match type_str {
        "float" => Some("0.0".to_string()),
        "int" => Some("0".to_string()),
        "uint" => Some("0u".to_string()),
        "bool" => Some("false".to_string()),
        t if t.starts_with("vec") || t.starts_with("mat") => Some(format!("{}(0.0)", t)),
        t if t.starts_with("ivec") => Some(format!("{}(0)", t)),
        t if t.starts_with("uvec") => Some(format!("{}(0u)", t)),
        t if t.starts_with("bvec") => Some(format!("{}(false)", t)),
        t => {
            if let Some(struct_def) = struct_defs.get(t) {
                let member_inits: Vec<String> = struct_def
                    .members
                    .iter()
                    .filter_map(|member| {
                        default_value(
                            &member.type_name,
                            member.array_specifier.as_deref(),
                            struct_defs,
                        )
                    })
                    .collect();

                if member_inits.len() == struct_def.members.len() {
                    Some(format!("{}({})", t, member_inits.join(", ")))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}
