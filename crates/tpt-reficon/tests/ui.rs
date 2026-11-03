//! Compile-fail checks: a literal that breaks a refinement must not compile.

#[test]
fn refined_literal_is_checked_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/refined_bad_literal.rs");
}
