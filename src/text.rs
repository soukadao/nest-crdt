use serde::{Deserialize, Serialize};

use crate::CrdtValue;
use crate::hlc::HLC;

/// Unique operation identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpId {
    pub node_id: u128,
    pub timestamp: HLC,
}

impl OpId {
    fn priority_cmp(&self, other: &OpId) -> std::cmp::Ordering {
        // Higher timestamp wins (most recent first).
        self.timestamp.cmp(&other.timestamp).reverse()
    }
}

/// CRDT operation for text editing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TextOp {
    Insert {
        id: OpId,
        parent: Option<OpId>,
        ch: char,
    },
    Delete {
        target: OpId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Element {
    id: OpId,
    ch: char,
    deleted: bool,
    parent: Option<OpId>,
}

/// RGA-based text CRDT.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextCrdt {
    node_id: u128,
    clock: HLC,
    elements: Vec<Element>,
    ops_log: Vec<TextOp>,
}

impl TextCrdt {
    pub fn new(node_id: u128) -> Self {
        TextCrdt {
            node_id,
            clock: HLC::new(node_id),
            elements: Vec::new(),
            ops_log: Vec::new(),
        }
    }

    pub fn from_text(node_id: u128, text: &str) -> Self {
        let mut crdt = Self::new(node_id);
        for ch in text.chars() {
            crdt.insert(crdt.len(), ch);
        }
        crdt
    }

    pub fn node_id(&self) -> u128 {
        self.node_id
    }

    /// Number of visible (non-deleted) characters.
    pub fn len(&self) -> usize {
        self.elements.iter().filter(|e| !e.deleted).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Render the CRDT to a plain string.
    pub fn to_string(&self) -> String {
        self.elements
            .iter()
            .filter(|e| !e.deleted)
            .map(|e| e.ch)
            .collect()
    }

    /// Insert a character at a visible position.
    pub fn insert(&mut self, pos: usize, ch: char) -> TextOp {
        let id = OpId {
            node_id: self.node_id,
            timestamp: self.clock.now(),
        };

        let parent = if pos == 0 {
            None
        } else {
            let mut count = 0;
            let mut parent_id = None;
            for elem in &self.elements {
                if elem.deleted {
                    continue;
                }
                count += 1;
                if count == pos {
                    parent_id = Some(elem.id.clone());
                    break;
                }
            }
            parent_id
        };

        let op = TextOp::Insert {
            id: id.clone(),
            parent: parent.clone(),
            ch,
        };

        self.integrate_insert(id, parent, ch);
        self.ops_log.push(op.clone());
        op
    }

    /// Delete a character at a visible position.
    pub fn delete(&mut self, pos: usize) -> Option<TextOp> {
        let mut count = 0;
        let mut target_id = None;
        for elem in &self.elements {
            if elem.deleted {
                continue;
            }
            if count == pos {
                target_id = Some(elem.id.clone());
                break;
            }
            count += 1;
        }

        let target = target_id?;
        let op = TextOp::Delete {
            target: target.clone(),
        };
        self.integrate_delete(&target);
        self.ops_log.push(op.clone());
        Some(op)
    }

    /// Apply a remote operation.
    pub fn apply(&mut self, op: &TextOp) {
        match op {
            TextOp::Insert { id, parent, ch } => {
                if self.find_element(id).is_none() {
                    self.clock.receive(&id.timestamp);
                    self.integrate_insert(id.clone(), parent.clone(), *ch);
                    self.ops_log.push(op.clone());
                }
            }
            TextOp::Delete { target } => {
                self.integrate_delete(target);
                self.ops_log.push(op.clone());
            }
        }
    }

    /// Merge all operations from another CRDT instance.
    pub fn merge(&mut self, other: &TextCrdt) {
        for op in &other.ops_log {
            match op {
                TextOp::Insert { id, .. } => {
                    if self.find_element(id).is_none() {
                        self.apply(op);
                    }
                }
                TextOp::Delete { target } => {
                    if let Some(elem) = self.find_element(target) {
                        if !elem.deleted {
                            self.apply(op);
                        }
                    }
                }
            }
        }
    }

    /// Get operations since a given log index.
    pub fn ops_since(&self, from_index: usize) -> &[TextOp] {
        if from_index >= self.ops_log.len() {
            &[]
        } else {
            &self.ops_log[from_index..]
        }
    }

    pub fn ops_count(&self) -> usize {
        self.ops_log.len()
    }

    /// Create a fork for a different node.
    pub fn fork(&self, new_node_id: u128) -> Self {
        let mut forked = self.clone();
        forked.node_id = new_node_id;
        forked.clock = HLC::new(new_node_id);
        forked.clock.receive(&self.clock);
        forked
    }

    /// Garbage-collect tombstoned elements.
    pub fn gc(&mut self) {
        self.elements.retain(|e| !e.deleted);
        self.ops_log
            .retain(|op| !matches!(op, TextOp::Delete { .. }));
    }

    pub fn tombstone_count(&self) -> usize {
        self.elements.iter().filter(|e| e.deleted).count()
    }

    /// Generate CRDT operations from a text diff.
    pub fn apply_diff(&mut self, old: &str, new: &str) -> Vec<TextOp> {
        let mut ops = Vec::new();

        // Simple diff: find common prefix, common suffix, replace middle
        let old_chars: Vec<char> = old.chars().collect();
        let new_chars: Vec<char> = new.chars().collect();

        let common_prefix = old_chars
            .iter()
            .zip(new_chars.iter())
            .take_while(|(a, b)| a == b)
            .count();

        let common_suffix = old_chars[common_prefix..]
            .iter()
            .rev()
            .zip(new_chars[common_prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();

        let delete_end = old_chars.len() - common_suffix;
        let insert_end = new_chars.len() - common_suffix;

        // Delete characters from old[common_prefix..delete_end] (in reverse)
        for i in (common_prefix..delete_end).rev() {
            if let Some(op) = self.delete(i) {
                ops.push(op);
            }
        }

        // Insert characters from new[common_prefix..insert_end]
        for (offset, &ch) in new_chars[common_prefix..insert_end].iter().enumerate() {
            let op = self.insert(common_prefix + offset, ch);
            ops.push(op);
        }

        ops
    }

    fn integrate_insert(&mut self, id: OpId, parent: Option<OpId>, ch: char) {
        let new_elem = Element {
            id: id.clone(),
            ch,
            deleted: false,
            parent: parent.clone(),
        };

        if self.elements.is_empty() {
            self.elements.push(new_elem);
            return;
        }

        let start_idx = match &parent {
            None => 0,
            Some(pid) => match self.elements.iter().position(|e| e.id == *pid) {
                Some(pos) => pos + 1,
                None => {
                    self.elements.push(new_elem);
                    return;
                }
            },
        };

        let mut insert_idx = start_idx;
        while insert_idx < self.elements.len() {
            let existing = &self.elements[insert_idx];
            if existing.parent != parent {
                break;
            }
            if id.priority_cmp(&existing.id) == std::cmp::Ordering::Greater {
                break;
            }
            insert_idx += 1;
        }

        self.elements.insert(insert_idx, new_elem);
    }

    fn integrate_delete(&mut self, target: &OpId) {
        if let Some(elem) = self.elements.iter_mut().find(|e| e.id == *target) {
            elem.deleted = true;
        }
    }

    fn find_element(&self, id: &OpId) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == *id)
    }
}

impl CrdtValue for TextCrdt {
    type Op = TextOp;
    fn apply(&mut self, op: &Self::Op) {
        TextCrdt::apply(self, op);
    }
    fn merge(&mut self, other: &Self) {
        TextCrdt::merge(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let mut crdt = TextCrdt::new(1);
        crdt.insert(0, 'H');
        crdt.insert(1, 'i');
        assert_eq!(crdt.to_string(), "Hi");
    }

    #[test]
    fn test_convergence() {
        let base = TextCrdt::from_text(1, "Hello");
        let mut a = base.fork(1);
        let mut b = base.fork(2);

        a.insert(5, '!');
        b.insert(5, '?');

        a.merge(&b);
        b.merge(&a);
        assert_eq!(a.to_string(), b.to_string());
    }

    #[test]
    fn test_apply_diff() {
        let mut crdt = TextCrdt::from_text(1, "Hello World");
        let ops = crdt.apply_diff("Hello World", "Hello Nest!");
        assert!(!ops.is_empty());
        assert_eq!(crdt.to_string(), "Hello Nest!");
    }

    #[test]
    fn test_three_way_convergence() {
        let base = TextCrdt::from_text(1, "ABCDE");
        let mut s1 = base.fork(1);
        let mut s2 = base.fork(2);
        let mut s3 = base.fork(3);

        s1.insert(2, 'X');
        s2.insert(4, 'Y');
        s3.delete(2); // delete 'C'

        s1.merge(&s2);
        s1.merge(&s3);

        let result = s1.to_string();
        assert!(result.contains('A'));
        assert!(result.contains('B'));
        assert!(result.contains('X'));
        assert!(!result.contains('C'));
        assert!(result.contains('D'));
        assert!(result.contains('Y'));
        assert!(result.contains('E'));
    }
}
