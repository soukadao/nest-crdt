use serde::{Deserialize, Serialize};

use crate::CrdtValue;
use crate::hlc::HLC;

/// Last-Writer-Wins Register.
/// Conflict resolution via HLC timestamp comparison.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LwwRegister<T: Clone> {
    pub value: T,
    pub timestamp: HLC,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LwwOp<T: Clone> {
    pub value: T,
    pub timestamp: HLC,
}

impl<T: Clone + Serialize + serde::de::DeserializeOwned> LwwRegister<T> {
    pub fn new(value: T, clock: &mut HLC) -> Self {
        LwwRegister {
            timestamp: clock.now(),
            value,
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn set(&mut self, value: T, clock: &mut HLC) -> LwwOp<T> {
        let ts = clock.now();
        self.value = value.clone();
        self.timestamp = ts.clone();
        LwwOp {
            value,
            timestamp: ts,
        }
    }

    pub fn apply(&mut self, op: &LwwOp<T>) {
        if op.timestamp > self.timestamp {
            self.value = op.value.clone();
            self.timestamp = op.timestamp.clone();
        }
    }

    pub fn merge(&mut self, other: &LwwRegister<T>) {
        if other.timestamp > self.timestamp {
            self.value = other.value.clone();
            self.timestamp = other.timestamp.clone();
        }
    }
}

impl<T: Clone + Serialize + serde::de::DeserializeOwned> CrdtValue for LwwRegister<T> {
    type Op = LwwOp<T>;
    fn apply(&mut self, op: &Self::Op) {
        LwwRegister::apply(self, op);
    }
    fn merge(&mut self, other: &Self) {
        LwwRegister::merge(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_writer_wins() {
        let mut clock_a = HLC::new(1);
        let mut clock_b = HLC::new(2);

        let mut reg_a = LwwRegister::new("initial".to_string(), &mut clock_a);
        let mut reg_b = reg_a.clone();

        let op_a = reg_a.set("from A".to_string(), &mut clock_a);
        let op_b = reg_b.set("from B".to_string(), &mut clock_b);

        // Apply both ops to both replicas
        reg_a.apply(&op_b);
        reg_b.apply(&op_a);

        // Both should converge to the same value
        assert_eq!(reg_a.get(), reg_b.get());
    }
}
