use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::CrdtValue;
use crate::hlc::HLC;

/// Unique token for add/remove operations.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token {
    pub node_id: u128,
    pub timestamp: HLC,
}

/// Observed-Remove Set CRDT.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetCrdt<T: Ord + Clone> {
    node_id: u128,
    elements: BTreeMap<T, HashSet<Token>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SetOp<T: Ord + Clone> {
    Add { value: T, token: Token },
    Remove { value: T, tokens: HashSet<Token> },
}

impl<T: Ord + Clone + Serialize + serde::de::DeserializeOwned> SetCrdt<T> {
    pub fn new(node_id: u128) -> Self {
        SetCrdt {
            node_id,
            elements: BTreeMap::new(),
        }
    }

    pub fn contains(&self, value: &T) -> bool {
        self.elements
            .get(value)
            .is_some_and(|tokens| !tokens.is_empty())
    }

    pub fn add(&mut self, value: T, clock: &mut HLC) -> SetOp<T> {
        let token = Token {
            node_id: self.node_id,
            timestamp: clock.now(),
        };
        self.elements
            .entry(value.clone())
            .or_default()
            .insert(token.clone());
        SetOp::Add { value, token }
    }

    pub fn remove(&mut self, value: &T, _clock: &mut HLC) -> Option<SetOp<T>> {
        let tokens = self.elements.get(value)?;
        if tokens.is_empty() {
            return None;
        }
        let removed_tokens = tokens.clone();
        self.elements.remove(value);
        Some(SetOp::Remove {
            value: value.clone(),
            tokens: removed_tokens,
        })
    }

    pub fn apply_op(&mut self, op: &SetOp<T>) {
        match op {
            SetOp::Add { value, token } => {
                self.elements
                    .entry(value.clone())
                    .or_default()
                    .insert(token.clone());
            }
            SetOp::Remove { value, tokens } => {
                if let Some(existing) = self.elements.get_mut(value) {
                    for token in tokens {
                        existing.remove(token);
                    }
                    if existing.is_empty() {
                        self.elements.remove(value);
                    }
                }
            }
        }
    }

    pub fn merge(&mut self, other: &SetCrdt<T>) {
        for (value, tokens) in &other.elements {
            let entry = self.elements.entry(value.clone()).or_default();
            for token in tokens {
                entry.insert(token.clone());
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements
            .iter()
            .filter(|(_, tokens)| !tokens.is_empty())
            .map(|(value, _)| value)
    }

    pub fn len(&self) -> usize {
        self.elements
            .values()
            .filter(|tokens| !tokens.is_empty())
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Ord + Clone + Serialize + serde::de::DeserializeOwned> CrdtValue for SetCrdt<T> {
    type Op = SetOp<T>;
    fn apply(&mut self, op: &Self::Op) {
        self.apply_op(op);
    }
    fn merge(&mut self, other: &Self) {
        SetCrdt::merge(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_remove() {
        let mut clock = HLC::new(1);
        let mut set = SetCrdt::new(1);
        set.add("a".to_string(), &mut clock);
        set.add("b".to_string(), &mut clock);
        assert!(set.contains(&"a".to_string()));
        assert_eq!(set.len(), 2);

        set.remove(&"a".to_string(), &mut clock);
        assert!(!set.contains(&"a".to_string()));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_concurrent_add_remove() {
        let mut clock_a = HLC::new(1);
        let mut clock_b = HLC::new(2);

        let mut set_a = SetCrdt::new(1);
        let mut set_b = SetCrdt::new(2);

        // A adds "x"
        let op_add = set_a.add("x".to_string(), &mut clock_a);
        set_b.apply_op(&op_add);

        // A removes "x", B concurrently adds "x"
        let op_remove = set_a.remove(&"x".to_string(), &mut clock_a).unwrap();
        let op_add2 = set_b.add("x".to_string(), &mut clock_b);

        set_a.apply_op(&op_add2);
        set_b.apply_op(&op_remove);

        // "x" should still exist (add wins over concurrent remove in OR-Set)
        assert!(set_a.contains(&"x".to_string()));
        assert!(set_b.contains(&"x".to_string()));
    }
}
