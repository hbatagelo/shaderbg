use pretty_assertions::assert_eq;

use super::super::{to_glsl_version, DIFF_RESERVED_WORDS_3_0_ES_REV_2, DIFF_RESERVED_WORDS_4_2};

#[test]
fn test_rename_reserved_4_2() {
    for &word in &DIFF_RESERVED_WORDS_4_2 {
        let expected = format!("{}_", word);
        let source = to_glsl_version(word, (4, 2), false).unwrap();
        assert_eq!(source, expected);
    }
}

#[test]
fn test_rename_reserved_3_0_es() {
    for &word in &DIFF_RESERVED_WORDS_3_0_ES_REV_2 {
        let expected = format!("{}_", word);
        let source = to_glsl_version(word, (3, 0), true).unwrap();
        assert_eq!(source, expected);
    }
}
