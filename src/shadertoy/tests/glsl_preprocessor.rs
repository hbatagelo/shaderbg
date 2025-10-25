mod define_and_macros {
    use pretty_assertions::assert_eq;

    use crate::shadertoy::glsl_preprocessor::preprocess;

    #[test]
    fn test_basic_definition() {
        let source = r#"
#define FOO .4
const float baz = FOO;
"#;
        let expected = "const float baz = .4;";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_nested_definition() {
        let source = r#"
#define FOO .4
#define BAR (FOO * 4.)
const float baz = BAR / float(N);
"#;
        let expected = "const float baz = (.4 * 4.) / float(N);";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_multiple_expansions_in_the_same_line() {
        let source = r#"
#define C(d) (iC, (d))
vec2 d = C( t.ww); vec2 e = C( t.wy);
"#;
        let expected = "vec2 d = (iC, (t.ww)); vec2 e = (iC, (t.wy));";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_function_like_macro_with_args() {
        let source = r#"
#define r(v,t) { float a = t; v *= 2.0; }
r(t.xy,u.x);
"#;
        let expected = "{ float a = u.x; t.xy *= 2.0; };";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_define_multiple_lines() {
        let source = r#"
#define DEF(struct_name, name, type, count)\
    const struct struct_name { type data[count]; }\
    name = struct_name(type[count]
DEF(Foo, b, int, 10));
"#;
        let expected = "const struct Foo { int data[10]; }    b = Foo(int[10]);";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_define_with_line_breaks_in_invocation() {
        let source = r#"
#define WRAP(struct_name, name, type, count)\
    struct struct_name { type d[count]; } name = struct_name(type[count]

#define V(x,y) vec2(x,y),

WRAP(Foo,f,vec2,N+1)(V(.6,0
)V(-.7,.1)V(2
,2)V
(-.1,.2)
"#;
        let expected = "struct Foo { vec2 d[N+1]; } f = Foo(vec2[N+1](vec2(.6,0),vec2(-.7,.1),vec2(2,2),vec2(-.1,.2),";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_undef() {
        let source = r#"
void foo()
{
    float x, y;
    #define DEF(a, b) def(a, b)

    x = min(x, DEF(1, 2));
    #undef DEF
    y = min(y, DEF(1, 2));
}
"#;
        let expected = r#"
void foo()
{
    float x, y;

    x = min(x, def(1, 2));
    y = min(y, DEF(1, 2));
}
"#
        .trim();
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }
}

mod conditionals {
    mod if_directive {
        use pretty_assertions::assert_eq;

        use crate::shadertoy::glsl_preprocessor::preprocess;

        #[test]
        fn test_if_with_defined_constant() {
            let source = r#"
#define DEF_A 1
#define DEF_B 0
#define DEF_C 2
#if DEF_A
vec3 a;
#endif
#if DEF_B
vec3 b;
#endif
#if DEF_C
vec3 c;
#endif
"#;
            let expected = "vec3 a;\nvec3 c;";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }

        #[test]
        fn test_if_with_literal_values() {
            let source = r#"
#if 1
#define ONE 1
#endif

#if 0
#define ZERO 0 // This should not be defined
#endif

int test_if() {
#if defined(ZERO)
  return ZERO;
#else
  return ONE;
#endif
}
"#;
            let expected = "int test_if() {\n  return 1;\n}";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }

        #[test]
        fn test_if_with_or_expression() {
            let source = r#"
#define DEF
#if defined(__cplusplus) || defined(DEF)
#define baz Baz
#endif
mat3 m = (foo(baz * -12.)) * (bar);
"#;
            let expected = "mat3 m = (foo(Baz * -12.)) * (bar);";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }

        #[test]
        fn test_if_with_not_operator() {
            let source = r#"
#define FLAG 0
#if !FLAG
vec3 a;
#else
    return true;
#endif
#if !0
vec3 b;
#endif
"#;
            let expected = "vec3 a;\nvec3 b;";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }

        #[test]
        fn test_complex_logical_expressions() {
            let source = r#"
#define DEBUG_TEX 2
#define NUM_MAT 10
#if defined(DEBUG_TEX) && (DEBUG_TEX >= 0) && (DEBUG_TEX < NUM_MAT)
int active1;
#else
int inactive1;
#endif

#define DEBUG_LM 3
#define DEBUG_MP 3
#if DEBUG_LM >= 2 && DEBUG_MP == 3
int active2;
#else
int inactive2;
#endif

#define VALUE 250
#if VALUE < 300
int active3;
#else
int inactive3;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
            assert!(!result.contains("inactive1"));
            assert!(!result.contains("inactive2"));
            assert!(!result.contains("inactive3"));
        }

        #[test]
        fn test_arithmetic_operations() {
            let source = r#"
#define A 10
#define B 5
#define C 3

#if A + B * C == 25
int active1;
#endif

#if A - B < C
int inactive1;
#endif

#if (A << 1) == 20
int active2;
#endif

#if (A >> 1) == 5
int active3;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
            assert!(!result.contains("inactive1"));
        }

        #[test]
        fn test_relational_operations() {
            let source = r#"
#define X 100
#define Y 200

#if X < Y
int active1;
#endif

#if X > Y
int inactive1;
#endif

#if X <= 100
int active2;
#endif

#if Y >= 200
int active3;
#endif

#if X == 100
int active4;
#endif

#if Y != 200
int inactive2;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
            assert!(result.contains("active4"));
            assert!(!result.contains("inactive1"));
            assert!(!result.contains("inactive2"));
        }

        #[test]
        fn test_logical_operations() {
            let source = r#"
#define FLAG1 1
#define FLAG2 0
#define FLAG3 1

#if FLAG1 && !FLAG2
int active1;
#endif

#if FLAG1 || FLAG2
int active2;
#endif

#if FLAG1 && FLAG2
int inactive1;
#endif

#if FLAG3 && (FLAG1 || FLAG2)
int active3;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
            assert!(!result.contains("inactive1"));
        }

        #[test]
        fn test_complex_nested_bitwise_expressions() {
            let source = r#"
#define A 5
#define B 10
#define C 3
#define D 7
#define E 1

#if ((A * B) + (C << 2)) == (50 + 12) && (D % C == E)
int active1;
#else
int inactive1;
#endif

#if (A | B) == 15 && (C & D) == 3
int active2;
#endif

#if (A ^ B) == 15
int active3;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
            assert!(!result.contains("inactive1"));
        }

        #[test]
        fn test_function_like_macros_in_conditions() {
            let source = r#"
#define VALUE 100
#define DOUBLE(x) (x * 2)
#define SQUARE(x) (x * x)

#if DOUBLE(VALUE) == 200
int active1;
#endif

#if SQUARE(10) == 100
int active2;
#endif

#if SQUARE(VALUE) > DOUBLE(VALUE)
int active3;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
        }

        #[test]
        fn test_division_by_zero() {
            let source = r#"
#define A 10
#define B 0

#if A / B > 0
int inactive1;
#else
int active1;
#endif

#if A % B != 0
int inactive2;
#else
int active2;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(!result.contains("inactive1"));
            assert!(!result.contains("inactive2"));
        }

        #[test]
        fn test_hex_and_octal_numbers() {
            let source = r#"
#define HEX_VAL 0x1F
#define OCT_VAL 077

#if HEX_VAL == 31
int active1;
#endif

#if OCT_VAL == 63
int active2;
#endif

#if HEX_VAL + OCT_VAL == 94
int active3;
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("active1"));
            assert!(result.contains("active2"));
            assert!(result.contains("active3"));
        }
    }

    mod ifdef_directive {
        use pretty_assertions::assert_eq;

        use crate::shadertoy::glsl_preprocessor::preprocess;

        #[test]
        fn test_ifdef_takes_defined_branch() {
            let source = r#"
#define DEF
#if defined(DEF)
#define VAL 1
#endif
int x = VAL;
"#;
            let expected = "int x = 1;";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }

        #[test]
        fn test_ifdef_takes_else_branch() {
            let source = r#"
#ifdef FAKE_FLAG
  // This block should be removed
  vec4 color = vec4(1.0, 0.0, 0.0, 1.0);
  #define SHOULD_BE_SKIPPED
#else
  // This block should be kept
  #define PI 3.14159
#endif
float bar() {
    return PI;
}
"#;
            let expected = "float bar() {\n    return 3.14159;\n}";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }
    }

    mod ifndef_directive {
        use crate::shadertoy::glsl_preprocessor::preprocess;

        #[test]
        fn test_ifndef_takes_branch_when_not_defined() {
            let source = r#"
#ifndef TEST_MACRO
void main() {}
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("void main() {}"));
        }

        #[test]
        fn test_ifndef_skips_branch_when_defined() {
            let source = r#"
#define TEST_MACRO
#ifndef TEST_MACRO
#error This should not be reached
#endif
"#;
            let result = preprocess(source);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().trim(), "");
        }

        #[test]
        fn test_nested_ifndef() {
            let source = r#"
#ifndef OUTER
#ifndef INNER
void main() {}
#endif
#endif
"#;
            let result = preprocess(source).unwrap();
            assert!(result.contains("void main() {}"));
        }
    }

    /// Tests for the #elif directive.
    mod elif_directive {
        use pretty_assertions::assert_eq;

        use crate::shadertoy::glsl_preprocessor::preprocess;

        #[test]
        fn test_elif_chain() {
            let source = r#"
#define OPTION_B

#if defined(OPTION_A)
    vec4 a = vec4(1.0, 0.0, 0.0, 1.0);
#elif defined(OPTION_B)
    vec4 a = vec4(0.0, 1.0, 0.0, 1.0);
#else
    vec4 a = vec4(0.0, 0.0, 1.0, 1.0);
#endif
"#;
            let expected = "vec4 a = vec4(0.0, 1.0, 0.0, 1.0);";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }

        #[test]
        fn test_if_elif_else_fallback() {
            let source = r#"
#if 0
vec4 d;
#elif 1
vec4 e;
#endif
"#;
            let expected = "vec4 e;";
            assert_eq!(preprocess(source).unwrap().trim(), expected);
        }
    }
}

mod error_directive {
    use pretty_assertions::assert_eq;

    use crate::{
        renderer::shader::ShaderError::PreprocessError, shadertoy::glsl_preprocessor::preprocess,
    };

    #[test]
    fn test_error_basic() {
        let source = r#"
#define DEBUG 1
#if DEBUG
    #error "Debug mode is enabled"
#endif
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("Debug mode is enabled".to_string(), 4)
        );
    }

    #[test]
    fn test_error_no_quotes() {
        let source = r#"
#error This is an error without quotes
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("This is an error without quotes".to_string(), 2)
        );
    }

    #[test]
    fn test_error_single_quotes() {
        let source = r#"
#error 'Single quoted error message'
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("Single quoted error message".to_string(), 2)
        );
    }

    #[test]
    fn test_error_empty_message() {
        let source = r#"
#error
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("Error directive encountered".to_string(), 2)
        );
    }

    #[test]
    fn test_error_in_inactive_branch() {
        let source = r#"
#define DEBUG 0
#if DEBUG
    #error "This should not trigger"
#endif
void main() {
    // This should be processed
}
"#;
        let result = preprocess(source);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("void main()"));
        assert!(!output.contains("#error"));
    }

    #[test]
    fn test_error_with_macro_expansion() {
        let source = r#"
#define ERROR_MSG "Macro expanded error"
#error ERROR_MSG
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        // Note: Macros are not expanded in #error messages
        assert_eq!(error, PreprocessError("ERROR_MSG".to_string(), 3));
    }

    #[test]
    fn test_error_in_nested_conditionals() {
        let source = r#"
#define FEATURE_A 1
#define FEATURE_B 1
#ifdef FEATURE_A
    #ifdef FEATURE_B
        #error "Both features enabled"
    #endif
#endif
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("Both features enabled".to_string(), 6)
        );
    }

    #[test]
    fn test_error_with_elif() {
        let source = r#"
#define VERSION 2
#if VERSION == 1
    // Version 1 code
#elif VERSION == 2
    #error "Version 2 is deprecated"
#else
    // Other versions
#endif
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("Version 2 is deprecated".to_string(), 6)
        );
    }

    #[test]
    fn test_error_line_counting() {
        let source = r#"// Line 1
// Line 2
#define TEST 1
// Line 4
#if TEST
    // Line 6
    #error "Error on line 7"
#endif
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error, PreprocessError("Error on line 7".to_string(), 7));
    }

    #[test]
    fn test_multiple_errors_first_wins() {
        let source = r#"
#define CONDITION 1
#if CONDITION
    #error "First error"
    #error "Second error"
#endif
"#;
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error, PreprocessError("First error".to_string(), 4));
    }
}

mod bitwise_operator_tests {
    use crate::shadertoy::glsl_preprocessor::preprocess;

    #[test]
    fn test_bitwise_and_comprehensive() {
        let source = r#"
    #define A 12      // Binary: 1100
    #define B 10      // Binary: 1010
    #define C 0x0F    // 15 in decimal

    // 12 & 10 = 8
    #if A & B == 8
    int active1;
    #endif

    // 15 & 12 = 12
    #if C & A == 12
    int active2;
    #endif

    // Chain: 15 & 12 & 10 = 8
    #if C & A & B == 8
    int active3;
    #endif

    // Zero test
    #if A & 0 == 0
    int active4;
    #endif
    "#;

        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
    }

    #[test]
    fn test_bitwise_or_comprehensive() {
        let source = r#"
    #define A 12      // Binary: 1100
    #define B 3       // Binary: 0011
    #define C 10      // Binary: 1010

    // 12 | 3 = 15
    #if A | B == 15
    int active1;
    #endif

    // 12 | 10 = 14
    #if A | C == 14
    int active2;
    #endif

    // Chain: 12 | 3 | 10 = 15
    #if A | B | C == 15
    int active3;
    #endif

    // Identity with 0
    #if A | 0 == A
    int active4;
    #endif
    "#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
    }

    #[test]
    fn test_bitwise_xor_comprehensive() {
        let source = r#"
    #define A 12      // Binary: 1100
    #define B 10      // Binary: 1010
    #define C 6       // Binary: 0110

    // 12 ^ 10 = 6
    #if A ^ B == 6
    int active1;
    #endif

    // 12 ^ 6 = 10
    #if A ^ C == 10
    int active2;
    #endif

    // Self XOR is zero
    #if A ^ A == 0
    int active3;
    #endif

    // XOR with 0 is identity
    #if A ^ 0 == A
    int active4;
    #endif

    // Chain: 12 ^ 10 ^ 6 = 0 (since 12 ^ 10 = 6, then 6 ^ 6 = 0)
    #if A ^ B ^ C == 0
    int active5;
    #endif
    "#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
        assert!(result.contains("active5"));
    }

    #[test]
    fn test_bitwise_not() {
        let source = r#"
#define ZERO 0
#define ONE 1
#define A 10      // Binary: 1010

// ~0 should be all 1s (non-zero)
#if ~ZERO != 0
int active1;
#endif

// ~(~A) should equal A
#if ~~A == A
int active2;
#endif

// For 32-bit: ~10 = 0xFFFFFFF5 = -11 in two's complement
#if ~A == -11
int active3;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
    }

    #[test]
    fn test_shift_operators_comprehensive() {
        let source = r#"
#define A 8
#define B 16
#define C 2

// Left shift: 8 << 2 = 32
#if A << C == 32
int active1;
#endif

// Right shift: 16 >> 2 = 4
#if B >> C == 4
int active2;
#endif

// Shift by 0 is identity
#if A << 0 == A
int active3;
#endif

#if B >> 0 == B
int active4;
#endif

// Chain shifts: (8 << 1) >> 1 = 16 >> 1 = 8
#if (A << 1) >> 1 == A
int active5;
#endif

// Large shift
#if 1 << 10 == 1024
int active6;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
        assert!(result.contains("active5"));
        assert!(result.contains("active6"));
    }
}

mod unary_operator_tests {
    use crate::shadertoy::glsl_preprocessor::preprocess;

    #[test]
    fn test_logical_not_comprehensive() {
        let source = r#"
#define TRUE 1
#define FALSE 0
#define NONZERO 42
#define NEGATIVE -5

// !1 = 0
#if !TRUE == 0
int active1;
#endif

// !0 = 1
#if !FALSE == 1
int active2;
#endif

// !42 = 0 (any non-zero is truthy)
#if !NONZERO == 0
int active3;
#endif

// !(-5) = 0 (negative non-zero is truthy)
#if !NEGATIVE == 0
int active4;
#endif

// Double negation: !!x converts to 0 or 1
#if !!TRUE == 1
int active5;
#endif

#if !!FALSE == 0
int active6;
#endif

#if !!NONZERO == 1
int active7;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
        assert!(result.contains("active5"));
        assert!(result.contains("active6"));
        assert!(result.contains("active7"));
    }

    #[test]
    fn test_unary_minus() {
        let source = r#"
#define POS 10
#define NEG -5
#define ZERO 0

// Negate positive
#if -POS == -10
int active1;
#endif

// Negate negative (double negative)
#if -NEG == 5
int active2;
#endif

// Negate zero
#if -ZERO == 0
int active3;
#endif

// Chain: -(-10) = 10
#if --POS == POS
int active4;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
    }

    #[test]
    fn test_unary_plus() {
        let source = r#"
#define POS 15
#define NEG -7
#define ZERO 0

// Unary plus should be identity
#if +POS == POS
int active1;
#endif

#if +NEG == NEG
int active2;
#endif

#if +ZERO == ZERO
int active3;
#endif

// Chain: +(+15) = 15
#if ++POS == POS
int active4;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
    }
}

mod modulo_operator_tests {
    use crate::shadertoy::glsl_preprocessor::preprocess;

    #[test]
    fn test_modulo_comprehensive() {
        let source = r#"
#define A 17
#define B 5
#define C 3

// 17 % 5 = 2
#if A % B == 2
int active1;
#endif

// 17 % 3 = 2
#if A % C == 2
int active2;
#endif

// 5 % 3 = 2
#if B % C == 2
int active3;
#endif

// Modulo with 1
#if A % 1 == 0
int active4;
#endif

// Self modulo
#if A % A == 0
int active5;
#endif

// Modulo larger number
#if C % A == C
int active6;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
        assert!(result.contains("active5"));
        assert!(result.contains("active6"));
    }

    #[test]
    fn test_modulo_with_negative() {
        let source = r#"
#define POS 17
#define NEG -17
#define DIVISOR 5

// Positive modulo positive: 17 % 5 = 2
#if POS % DIVISOR == 2
int active1;
#endif

// Negative modulo positive: -17 % 5 = -2 (in C)
#if NEG % DIVISOR == -2
int active2;
#endif

// Test with negative divisor: 17 % -5 = 2 (in C)
#if POS % -DIVISOR == 2
int active3;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
    }
}

mod inequality_operator_tests {
    use crate::shadertoy::glsl_preprocessor::preprocess;

    #[test]
    fn test_inequality_edge_cases() {
        let source = r#"
#define A 100
#define B 100
#define C 99
#define D 101

// Equal values
#if A == B
int active1;
#endif

#if !(A != B)
int active2;
#endif

#if A <= B
int active3;
#endif

#if A >= B
int active4;
#endif

// Boundary cases
#if C < A
int active5;
#endif

#if A < D
int active6;
#endif

#if C <= A
int active7;
#endif

#if A >= C
int active8;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
        assert!(result.contains("active5"));
        assert!(result.contains("active6"));
        assert!(result.contains("active7"));
        assert!(result.contains("active8"));
    }

    #[test]
    fn test_inequality_with_negative_numbers() {
        let source = r#"
#define POS 10
#define NEG -10
#define ZERO 0

// Negative vs positive
#if NEG < POS
int active1;
#endif

#if POS > NEG
int active2;
#endif

// Zero comparisons
#if NEG < ZERO
int active3;
#endif

#if ZERO < POS
int active4;
#endif

#if POS != NEG
int active5;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
        assert!(result.contains("active5"));
    }
}

mod number_format_tests {
    use crate::shadertoy::glsl_preprocessor::preprocess;

    #[test]
    fn test_octal_edge_cases() {
        let source = r#"
// Test various octal numbers
#define OCT_8 010   // 8 in decimal
#define OCT_64 0100 // 64 in decimal
#define OCT_0 00    // 0 in decimal

#if OCT_8 == 8
int active1;
#endif

#if OCT_64 == 64
int active2;
#endif

#if OCT_0 == 0
int active3;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
    }

    #[test]
    fn test_hex_case_insensitive() {
        let source = r#"
#define HEX_LOWER 0xff
#define HEX_UPPER 0XFF
#define HEX_MIXED 0xAb

#if HEX_LOWER == 255
int active1;
#endif

#if HEX_UPPER == 255
int active2;
#endif

#if HEX_MIXED == 171
int active3;
#endif

#if HEX_LOWER == HEX_UPPER
int active4;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
    }
}

mod edge_case_tests {
    use crate::shadertoy::glsl_preprocessor::preprocess;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_operator_without_spaces() {
        let source = r#"
#define A 5
#define B 3

// Test operators without spaces
#if A+B==8
int active1;
#endif

#if A*B!=14
int active2;
#endif

#if A<<1>B
int active3;
#endif

#if A&&B||0
int active4;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
        assert!(result.contains("active3"));
        assert!(result.contains("active4"));
    }

    #[test]
    fn test_deeply_nested_parentheses() {
        let source = r#"
#define A 2
#define B 3
#define C 4

// (((2 + 3) * 4) - 8) / 3 = ((5 * 4) - 8) / 3 = (20 - 8) / 3 = 12 / 3 = 4
#if (((A + B) * C) - 8) / 3 == 4
int active1;
#endif

// Test multiple levels of grouping
#if ((A * B) + (C - B)) == (6 + 1)
int active2;
#endif
"#;
        let result = preprocess(source).unwrap();
        assert!(result.contains("active1"));
        assert!(result.contains("active2"));
    }

    #[test]
    fn test_malformed_expressions() {
        let sources = vec!["#if (", "#if )", "#if 5 +", "#if * 5", "#if 5 ++", "#if"];

        for source in sources {
            let result = preprocess(source);
            if let Ok(output) = result {
                assert_eq!(output.trim(), "");
            }
        }
    }
}

mod general {
    use pretty_assertions::assert_eq;

    use crate::{
        renderer::shader::ShaderError::PreprocessError, shadertoy::glsl_preprocessor::preprocess,
    };

    #[test]
    fn test_preprocessor_strips_comments() {
        let source = r#"
#define VALUE 5.0
vec3 v; // a vector

/* This is a block comment.
   It spans multiple lines.
   #define HIDDEN 1.0 <-- this should be stripped
*/

void main() {
    v = vec3(VALUE); /* set value */
}
"#;
        let expected = "vec3 v;  \n\n \n\nvoid main() {\n    v = vec3(5.0);  \n}";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_handles_space_after_hash() {
        let source = r#"
#  define OPTION_B

#   if defined(OPTION_A)
    vec4 a = vec4(1.0, 0.0, 0.0, 1.0);
# elif defined(OPTION_B)
    vec4 a = vec4(0.0, 1.0, 0.0, 1.0);
#   else
    vec4 a = vec4(0.0, 0.0, 1.0, 1.0);
# endif
"#;
        let expected = "vec4 a = vec4(0.0, 1.0, 0.0, 1.0);";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_ignores_unsupported_directives() {
        let source = r#"
#
#pragma debug(on)
#extension
#version
#line
"#;
        let expected = "";
        assert_eq!(preprocess(source).unwrap().trim(), expected);
    }

    #[test]
    fn test_errs_on_unknown_directive() {
        let source = "#unknown";
        let result = preprocess(source);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            error,
            PreprocessError("Unknown directive (unknown)".to_string(), 1)
        );
    }
}
