use pretty_assertions::assert_eq;

use super::super::glsl_initializer;

fn initialize(source: &str) -> String {
    glsl_initializer::initialize_uninitialized_variables(source).unwrap()
}

#[test]
fn test_basic() {
    let source = "float a, b=1.0, c; int x=10, y; uint z;";
    let expected = "float a = 0.0, b=1.0, c = 0.0; int x=10, y = 0; uint z = 0u;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_inside_function_definition() {
    let source = r#"
void main() { float a, b=1.0, c; }
int f(vec3 a, bvec2 b);
int f(vec3 a, bvec2 b) { vec2 x, y; }
vec3 f(vec3 r) { float a, b; }
vec3 f() { return vec3(0.5); }
vec3 f() { float x; }
vec3 f() {
    if (y > 1.0) { float x; }
    vec2 a, b;
    return a;
}
"#;
    let expected = r#"
void main() { float a = 0.0, b=1.0, c = 0.0; }
int f(vec3 a, bvec2 b);
int f(vec3 a, bvec2 b) { vec2 x = vec2(0.0), y = vec2(0.0); }
vec3 f(vec3 r) { float a = 0.0, b = 0.0; }
vec3 f() { return vec3(0.5); }
vec3 f() { float x = 0.0; }
vec3 f() {
    if (y > 1.0) { float x = 0.0; }
    vec2 a = vec2(0.0), b = vec2(0.0);
    return a;
}
"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_vector() {
    let source = "vec3 pos = vec3(1), n; vec2 a;";
    let expected = "vec3 pos = vec3(1), n = vec3(0.0); vec2 a = vec2(0.0);";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_matrix() {
    let source = "mat4 m;";
    let expected = "mat4 m = mat4(0.0);";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_define() {
    let source = "#define VEC3 vec3\nVEC3 a, b;";
    let expected = "vec3 a = vec3(0.0), b = vec3(0.0);";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_nested_define() {
    let source = "#define VEC3 vec3\n#define FLT3 VEC3\nFLT3 a;";
    let expected = "vec3 a = vec3(0.0);";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_initialized_after_comment() {
    let source = r#"
// Single-line comment
vec2 a; /* Multi-line
comment */ float b;
        "#;
    let expected = "vec2 a = vec2(0.0);   float b = 0.0;";
    assert_eq!(initialize(source), expected.trim());
}

#[test]
fn test_skip_const() {
    let source = r#"
const vec4 c[] = vec4[](vec4(0), vec4(1), vec4(0, 0, 1, 1), vec4(1, 1, 0, 1));
const float PI = 3.14; float a, b;
"#;
    let expected = r#"
const vec4 c[] = vec4[](vec4(0), vec4(1), vec4(0, 0, 1, 1), vec4(1, 1, 0, 1));
const float PI = 3.14; float a = 0.0, b = 0.0;
"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_skip_uniform() {
    let source = "uniform mat4 view; vec3 position;";
    let expected = "uniform mat4 view; vec3 position = vec3(0.0);";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_skip_in_out() {
    let source = "in vec3 position; out vec4 color; float a;";
    let expected = "in vec3 position; out vec4 color; float a = 0.0;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_skip_varying() {
    let source = "varying vec2 uv; float b;";
    let expected = "varying vec2 uv; float b = 0.0;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_skip_in_middle() {
    let source = "in vec3 position; float a;";
    let expected = "in vec3 position; float a = 0.0;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_function_call() {
    let source = "f(vec4(1,3,4,2));";
    let expected = "f(vec4(1,3,4,2));";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_function_in_expression() {
    let source = "float v = f() * 2.0;";
    let expected = "float v = f() * 2.0;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_mixed_declaration_and_call() {
    let source = "vec4 a; f(vec4(1,2,3,4));";
    let expected = "vec4 a = vec4(0.0); f(vec4(1,2,3,4));";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_multiline_declaration() {
    let source = r#"
void mainImage(out vec4 o,vec2 C){
    float
      i
    , j = 2.
    , d
    , z
    , a=42.0
    ;
}
"#;
    let expected = r#"
void mainImage(out vec4 o,vec2 C){
    float
      i = 0.0, j = 2., d = 0.0, z = 0.0, a=42.0;
}
"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_struct_declaration() {
    let source = r#"
struct foo
{
    bvec2 a;
    vec3 d;
    float t;
};
"#;
    let expected = r#"
struct foo
{
    bvec2 a;
    vec3 d;
    float t;
};
"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_struct_initialization() {
    let source = r#"
struct Foo
{
    vec3 a, b;
    float c;
};
Foo foo;"#;
    let expected = r#"
struct Foo
{
    vec3 a, b;
    float c;
};
Foo foo = Foo(vec3(0.0), vec3(0.0), 0.0);"#
        .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_struct_initialization_nested() {
    let source = r#"
struct Bar
{
    vec3 a; float b;
};
struct Foo
{
    vec3 a;
    mat3 b;
    float c, d;
    Bar e;
};
Foo foo;
Bar bar;"#;
    let expected = r#"
struct Bar
{
    vec3 a; float b;
};
struct Foo
{
    vec3 a;
    mat3 b;
    float c, d;
    Bar e;
};
Foo foo = Foo(vec3(0.0), mat3(0.0), 0.0, 0.0, Bar(vec3(0.0), 0.0));
Bar bar = Bar(vec3(0.0), 0.0);"#
        .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_const_struct_1() {
    let source = r#"
const struct Foo
{
    vec3 a;
    vec2 b;
}
foo = Foo(vec3(1,2,3), vec2(0,5));
const struct Bar
{
    vec3 a;
    vec2 b;
};
"#;
    let expected = r#"
const struct Foo
{
    vec3 a;
    vec2 b;
}
foo = Foo(vec3(1,2,3), vec2(0,5));
const struct Bar
{
    vec3 a;
    vec2 b;
};
"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_for_initialization() {
    let source = "for(vec3 a; ++i<42.; b+=.9*d) {}";
    let expected = "for(vec3 a = vec3(0.0); ++i<42.; b+=.9*d) {}";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_remove_empty_lines() {
    let source = "const vec2 a;\n\n   vec3 b = vec3(1);\n    \n \t\t  \nvec3 c = vec3(0);\n";
    let expected = "const vec2 a;\n   vec3 b = vec3(1);\nvec3 c = vec3(0);";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_vec3_array() {
    let source = "vec3[2] deltas;";
    let expected = "vec3[2] deltas = vec3[2](vec3(0.0), vec3(0.0));";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_array_of_scalars_1() {
    let source = "int[3] counts, a;";
    let expected = "int[3] counts = int[3](0, 0, 0), a = 0;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_array_of_scalars_2() {
    let source = "float arr[2], b;";
    let expected = "float arr[2] = float[2](0.0, 0.0), b = 0.0;";
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_multidimensional_array() {
    let source = "vec2[2][3] points;";
    let expected = r#"
vec2[2][3] points = vec2[2][3](vec2[2](vec2(0.0), vec2(0.0)), vec2[2](vec2(0.0), vec2(0.0)), vec2[2](vec2(0.0), vec2(0.0)));"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_array_of_structs() {
    let source = r#"
struct Point { vec2 pos; };
Point[3] points;"#;
    let expected = r#"
struct Point { vec2 pos; };
Point[3] points = Point[3](Point(vec2(0.0)), Point(vec2(0.0)), Point(vec2(0.0)));"#
        .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_multiline_initialized_arrays() {
    let source = r#"
int[] a = int[](1),
      b = int[](2);
mat3 f = mat3(0.0);
"#;
    let expected = r#"
int[] a = int[](1), b = int[](2);
mat3 f = mat3(0.0);
"#
    .trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_array_in_local_scope() {
    let source = "{ int Foo[]=int[3](int(3),int(3),int(6));";
    let expected = "{ int Foo[]=int[3](int(3),int(3),int(6));".trim();
    assert_eq!(initialize(source), expected);
}

#[test]
fn test_const_array_in_local_scope() {
    let source = "{ const int Foo[]=int[3](int(3),int(3),int(6));";
    let expected = "{ const int Foo[]=int[3](int(3),int(3),int(6));".trim();
    assert_eq!(initialize(source), expected);
}
