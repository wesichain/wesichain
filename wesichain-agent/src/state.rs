pub trait AgentState {
    type FinalOutput;
    type ScratchpadEntry;
    type StepId: Copy + Eq + std::fmt::Debug;

    fn user_input(&self) -> &str;
    fn append_scratchpad(&mut self, entry: Self::ScratchpadEntry);
    fn set_final_output(&mut self, out: Self::FinalOutput);
    fn step_count(&self) -> u32;
    fn correlation_id(&self) -> &str;
}
