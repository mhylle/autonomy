use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Deterministic RNG provider.
///
/// Derives per-system RNGs from a master seed so that each system
/// gets its own reproducible random stream. Same master seed always
/// produces the same simulation.
pub struct SimulationRng {
    master_seed: u64,
}

impl SimulationRng {
    pub fn new(master_seed: u64) -> Self {
        Self { master_seed }
    }

    /// Derive a deterministic RNG for a named system.
    ///
    /// The system name is hashed and mixed with the master seed to
    /// produce a unique but reproducible seed per system.
    pub fn system_rng(&self, system_name: &str) -> ChaCha8Rng {
        let system_seed = self.derive_seed(system_name);
        ChaCha8Rng::seed_from_u64(system_seed)
    }

    /// Derive a deterministic RNG for a system at a specific tick.
    ///
    /// Incorporates the tick number so that each tick gets a fresh
    /// but still reproducible RNG state.
    pub fn tick_rng(&self, system_name: &str, tick: u64) -> ChaCha8Rng {
        let base_seed = self.derive_seed(system_name);
        let tick_seed = base_seed.wrapping_add(tick.wrapping_mul(6_364_136_223_846_793_005));
        ChaCha8Rng::seed_from_u64(tick_seed)
    }

    pub fn master_seed(&self) -> u64 {
        self.master_seed
    }

    fn derive_seed(&self, system_name: &str) -> u64 {
        let mut hash: u64 = self.master_seed;
        for byte in system_name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn same_seed_same_sequence() {
        let rng_provider = SimulationRng::new(42);
        let mut rng1 = rng_provider.system_rng("movement");
        let mut rng2 = rng_provider.system_rng("movement");

        let values1: Vec<f64> = (0..10).map(|_| rng1.gen()).collect();
        let values2: Vec<f64> = (0..10).map(|_| rng2.gen()).collect();
        assert_eq!(values1, values2);
    }

    #[test]
    fn different_systems_different_sequences() {
        let rng_provider = SimulationRng::new(42);
        let mut rng1 = rng_provider.system_rng("movement");
        let mut rng2 = rng_provider.system_rng("feeding");

        let val1: f64 = rng1.gen();
        let val2: f64 = rng2.gen();
        assert_ne!(val1, val2);
    }

    #[test]
    fn tick_rng_is_deterministic() {
        let rng_provider = SimulationRng::new(42);
        let mut rng1 = rng_provider.tick_rng("movement", 100);
        let mut rng2 = rng_provider.tick_rng("movement", 100);

        let val1: f64 = rng1.gen();
        let val2: f64 = rng2.gen();
        assert_eq!(val1, val2);
    }

    #[test]
    fn different_ticks_different_values() {
        let rng_provider = SimulationRng::new(42);
        let mut rng1 = rng_provider.tick_rng("movement", 1);
        let mut rng2 = rng_provider.tick_rng("movement", 2);

        let val1: f64 = rng1.gen();
        let val2: f64 = rng2.gen();
        assert_ne!(val1, val2);
    }
}
