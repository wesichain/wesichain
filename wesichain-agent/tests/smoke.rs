#[test]
fn crate_exports_public_entrypoints() {
    let _ = std::any::type_name::<
        wesichain_agent::AgentRuntime<(), (), wesichain_agent::NoopPolicy, wesichain_agent::Idle>,
    >();
}
