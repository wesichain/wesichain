use uuid::Uuid;

pub trait Sampler: Send + Sync {
    fn should_sample(&self, run_id: Uuid) -> bool;
}

#[derive(Clone, Debug)]
pub struct ProbabilitySampler {
    pub rate: f64,
}

impl Sampler for ProbabilitySampler {
    fn should_sample(&self, run_id: Uuid) -> bool {
        if self.rate <= 0.0 {
            return false;
        }
        if self.rate >= 1.0 {
            return true;
        }
        let bytes = run_id.as_bytes();
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[0..8]);
        let hash = u64::from_le_bytes(buf);
        let ratio = (hash as f64) / (u64::MAX as f64);
        ratio < self.rate
    }
}
