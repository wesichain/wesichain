use wesichain_graph::{ExecutionConfig, ExecutionOptions};

#[test]
fn execution_config_defaults_and_merge() {
    let defaults = ExecutionConfig::default();
    assert_eq!(defaults.max_steps, Some(50));
    assert!(defaults.cycle_detection);
    assert_eq!(defaults.cycle_window, 20);

    let overrides = ExecutionOptions {
        max_steps: Some(5),
        cycle_detection: Some(false),
        cycle_window: Some(3),
        run_config: None,
        observer: None,
    };
    let merged = defaults.merge(&overrides);
    assert_eq!(merged.max_steps, Some(5));
    assert!(!merged.cycle_detection);
    assert_eq!(merged.cycle_window, 3);

    let merged_empty = defaults.merge(&ExecutionOptions::default());
    assert_eq!(merged_empty.max_steps, Some(50));
    assert!(merged_empty.cycle_detection);
    assert_eq!(merged_empty.cycle_window, 20);
}
