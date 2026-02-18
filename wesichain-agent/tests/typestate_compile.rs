#[test]
fn typestate_transitions_are_enforced_at_compile_time() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_idle_default.rs");
    t.compile_fail("tests/ui/invalid_non_idle_default.rs");
    t.compile_fail("tests/ui/invalid_act_from_idle.rs");
    t.compile_fail("tests/ui/invalid_complete_from_acting.rs");
    t.pass("tests/ui/valid_think_act_observe.rs");
}
