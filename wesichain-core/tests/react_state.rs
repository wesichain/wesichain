use serde_json::json;
use wesichain_core::{HasFinalOutput, HasUserInput, ReActStep, ScratchpadState, ToolCall};

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
struct DemoState {
    input: String,
    scratchpad: Vec<ReActStep>,
    final_output: Option<String>,
    iterations: u32,
}

impl ScratchpadState for DemoState {
    fn scratchpad(&self) -> &Vec<ReActStep> {
        &self.scratchpad
    }

    fn scratchpad_mut(&mut self) -> &mut Vec<ReActStep> {
        &mut self.scratchpad
    }

    fn iteration_count(&self) -> u32 {
        self.iterations
    }

    fn increment_iteration(&mut self) {
        self.iterations += 1;
    }
}

impl HasUserInput for DemoState {
    fn user_input(&self) -> &str {
        &self.input
    }
}

impl HasFinalOutput for DemoState {
    fn final_output(&self) -> Option<&str> {
        self.final_output.as_deref()
    }

    fn set_final_output(&mut self, value: String) {
        self.final_output = Some(value);
    }
}

#[test]
fn react_step_serde_roundtrip() {
    let step = ReActStep::Action(ToolCall {
        id: "call-1".to_string(),
        name: "calculator".to_string(),
        args: json!({"expression": "2+2"}),
    });
    let value = serde_json::to_value(&step).expect("serialize");
    let decoded: ReActStep = serde_json::from_value(value).expect("deserialize");
    assert!(matches!(decoded, ReActStep::Action(_)));
}

#[test]
fn demo_state_trait_impls_work() {
    let mut state = DemoState {
        input: "hi".to_string(),
        ..Default::default()
    };

    assert_eq!(state.user_input(), "hi");
    assert_eq!(state.final_output(), None);
    state.set_final_output("done".to_string());
    assert_eq!(state.final_output(), Some("done"));

    assert_eq!(state.iteration_count(), 0);
    state.increment_iteration();
    assert_eq!(state.iteration_count(), 1);

    state
        .scratchpad_mut()
        .push(ReActStep::FinalAnswer("ok".to_string()));
    assert!(matches!(
        state.scratchpad().first(),
        Some(ReActStep::FinalAnswer(_))
    ));
}
