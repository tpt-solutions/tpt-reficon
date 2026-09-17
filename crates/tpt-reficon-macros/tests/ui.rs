#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/basic.rs");
    t.pass("tests/ui/pass/named_field.rs");
    t.pass("tests/ui/pass/integer_boundary.rs");
    t.compile_fail("tests/ui/fail/unsatisfiable.rs");
    t.compile_fail("tests/ui/fail/disjunctive_unsatisfiable.rs");
    t.compile_fail("tests/ui/fail/integer_gap.rs");
    t.compile_fail("tests/ui/fail/out_of_domain.rs");
    t.compile_fail("tests/ui/fail/unknown_variable.rs");
    t.compile_fail("tests/ui/fail/nonlinear.rs");
    t.compile_fail("tests/ui/fail/multiple_fields.rs");
    t.compile_fail("tests/ui/fail/generic.rs");
}
