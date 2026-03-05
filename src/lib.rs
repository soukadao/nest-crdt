pub mod hlc;
pub mod lww;
pub mod map;
pub mod sequence;
pub mod set;
pub mod text;

/// Trait that all CRDT values must implement.
/// Enables composition (e.g., MapCRDT<V: CrdtValue>).
pub trait CrdtValue: Clone + serde::Serialize + serde::de::DeserializeOwned {
    type Op: Clone + serde::Serialize + serde::de::DeserializeOwned;
    fn apply(&mut self, op: &Self::Op);
    fn merge(&mut self, other: &Self);
}
