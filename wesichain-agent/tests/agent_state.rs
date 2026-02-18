use wesichain_agent::AgentState;

#[derive(Default)]
struct DemoState {
    input: String,
    steps: u32,
    corr: String,
}

impl AgentState for DemoState {
    type FinalOutput = String;
    type ScratchpadEntry = String;
    type StepId = u32;

    fn user_input(&self) -> &str {
        &self.input
    }

    fn append_scratchpad(&mut self, _entry: Self::ScratchpadEntry) {}

    fn set_final_output(&mut self, _out: Self::FinalOutput) {}

    fn step_count(&self) -> u32 {
        self.steps
    }

    fn correlation_id(&self) -> &str {
        &self.corr
    }
}

#[test]
fn agent_state_contract_compiles() {
    let s = DemoState::default();
    assert_eq!(s.step_count(), 0);
}
