use uuid::Uuid;

pub trait Sampler: Send + Sync {
    fn should_sample(&self, _run_id: Uuid) -> bool;
}

#[derive(Clone, Debug)]
pub struct ProbabilitySampler {
    pub rate: f64,
}

impl Sampler for ProbabilitySampler {
    fn should_sample(&self, _run_id: Uuid) -> bool {
        let _ = self.rate;
        false
    }
}
