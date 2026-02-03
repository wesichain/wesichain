use uuid::Uuid;
use wesichain_langsmith::{ProbabilitySampler, Sampler};

#[test]
fn sampler_is_deterministic_by_run_id() {
    let sampler = ProbabilitySampler { rate: 0.5 };
    let run_id = Uuid::new_v4();
    let first = sampler.should_sample(run_id);
    let second = sampler.should_sample(run_id);
    assert_eq!(first, second);
}

#[test]
fn sampler_respects_bounds() {
    let sampler = ProbabilitySampler { rate: 0.0 };
    assert!(!sampler.should_sample(Uuid::new_v4()));

    let sampler = ProbabilitySampler { rate: 1.0 };
    assert!(sampler.should_sample(Uuid::new_v4()));
}
