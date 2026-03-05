use serde::{Deserialize, Serialize};

use crate::CrdtValue;
use crate::hlc::HLC;

/// Unique identifier for sequence items.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeqId {
    pub node_id: u128,
    pub timestamp: HLC,
}

/// Append-mostly ordered list CRDT.
/// Used for comments, history entries, etc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SequenceCrdt<T: Clone> {
    node_id: u128,
    items: Vec<SeqItem<T>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SeqItem<T: Clone> {
    id: SeqId,
    value: T,
    deleted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SequenceOp<T: Clone> {
    Append { id: SeqId, value: T },
    Delete { target: SeqId },
}

impl<T: Clone + Serialize + serde::de::DeserializeOwned> SequenceCrdt<T> {
    pub fn new(node_id: u128) -> Self {
        SequenceCrdt {
            node_id,
            items: Vec::new(),
        }
    }

    pub fn append(&mut self, value: T, clock: &mut HLC) -> SequenceOp<T> {
        let id = SeqId {
            node_id: self.node_id,
            timestamp: clock.now(),
        };
        self.items.push(SeqItem {
            id: id.clone(),
            value: value.clone(),
            deleted: false,
        });
        SequenceOp::Append { id, value }
    }

    pub fn delete(&mut self, index: usize) -> Option<SequenceOp<T>> {
        let mut visible_idx = 0;
        for item in &mut self.items {
            if item.deleted {
                continue;
            }
            if visible_idx == index {
                item.deleted = true;
                return Some(SequenceOp::Delete {
                    target: item.id.clone(),
                });
            }
            visible_idx += 1;
        }
        None
    }

    pub fn apply_op(&mut self, op: &SequenceOp<T>) {
        match op {
            SequenceOp::Append { id, value } => {
                if !self.items.iter().any(|item| item.id == *id) {
                    self.items.push(SeqItem {
                        id: id.clone(),
                        value: value.clone(),
                        deleted: false,
                    });
                    // Sort by timestamp to maintain consistent order
                    self.items.sort_by(|a, b| a.id.timestamp.cmp(&b.id.timestamp));
                }
            }
            SequenceOp::Delete { target } => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == *target) {
                    item.deleted = true;
                }
            }
        }
    }

    pub fn merge(&mut self, other: &SequenceCrdt<T>) {
        for item in &other.items {
            if !self.items.iter().any(|i| i.id == item.id) {
                self.items.push(item.clone());
            } else if item.deleted {
                if let Some(local) = self.items.iter_mut().find(|i| i.id == item.id) {
                    local.deleted = true;
                }
            }
        }
        self.items
            .sort_by(|a, b| a.id.timestamp.cmp(&b.id.timestamp));
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items
            .iter()
            .filter(|item| !item.deleted)
            .map(|item| &item.value)
    }

    pub fn len(&self) -> usize {
        self.items.iter().filter(|item| !item.deleted).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Clone + Serialize + serde::de::DeserializeOwned> CrdtValue for SequenceCrdt<T> {
    type Op = SequenceOp<T>;
    fn apply(&mut self, op: &Self::Op) {
        self.apply_op(op);
    }
    fn merge(&mut self, other: &Self) {
        SequenceCrdt::merge(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_iterate() {
        let mut clock = HLC::new(1);
        let mut seq = SequenceCrdt::new(1);

        seq.append("first".to_string(), &mut clock);
        seq.append("second".to_string(), &mut clock);
        seq.append("third".to_string(), &mut clock);

        let items: Vec<&String> = seq.iter().collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], "first");
        assert_eq!(items[2], "third");
    }

    #[test]
    fn test_concurrent_append() {
        let mut clock_a = HLC::new(1);
        let mut clock_b = HLC::new(2);

        let mut seq_a = SequenceCrdt::new(1);
        let mut seq_b = SequenceCrdt::new(2);

        let op_a = seq_a.append("from A".to_string(), &mut clock_a);
        let op_b = seq_b.append("from B".to_string(), &mut clock_b);

        seq_a.apply_op(&op_b);
        seq_b.apply_op(&op_a);

        let items_a: Vec<&String> = seq_a.iter().collect();
        let items_b: Vec<&String> = seq_b.iter().collect();
        assert_eq!(items_a, items_b);
    }
}
