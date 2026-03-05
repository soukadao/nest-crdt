use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::CrdtValue;
use crate::hlc::HLC;

/// Map CRDT with observed-remove semantics.
/// Keys can be added, updated, and removed concurrently.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "V: Serialize",
    deserialize = "V: serde::de::DeserializeOwned"
))]
pub struct MapCrdt<V: CrdtValue> {
    node_id: u128,
    entries: BTreeMap<String, V>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "V: Serialize, V::Op: Serialize",
    deserialize = "V: serde::de::DeserializeOwned, V::Op: serde::de::DeserializeOwned"
))]
pub enum MapOp<V: CrdtValue> {
    Put { key: String, value: V },
    Remove { key: String },
    Update { key: String, op: V::Op },
}

impl<V: CrdtValue> MapCrdt<V> {
    pub fn new(node_id: u128) -> Self {
        MapCrdt {
            node_id,
            entries: BTreeMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries.get_mut(key)
    }

    pub fn put(&mut self, key: String, value: V, _clock: &mut HLC) -> MapOp<V> {
        self.entries.insert(key.clone(), value.clone());
        MapOp::Put { key, value }
    }

    pub fn remove(&mut self, key: &str, _clock: &mut HLC) -> Option<MapOp<V>> {
        self.entries.remove(key)?;
        Some(MapOp::Remove {
            key: key.to_string(),
        })
    }

    pub fn update(&mut self, key: &str, op: V::Op) -> Option<MapOp<V>> {
        let value = self.entries.get_mut(key)?;
        value.apply(&op);
        Some(MapOp::Update {
            key: key.to_string(),
            op,
        })
    }

    pub fn apply_op(&mut self, op: &MapOp<V>) {
        match op {
            MapOp::Put { key, value } => {
                self.entries.insert(key.clone(), value.clone());
            }
            MapOp::Remove { key } => {
                self.entries.remove(key);
            }
            MapOp::Update { key, op } => {
                if let Some(value) = self.entries.get_mut(key) {
                    value.apply(op);
                }
            }
        }
    }

    pub fn merge(&mut self, other: &MapCrdt<V>) {
        for (key, other_value) in &other.entries {
            if let Some(local_value) = self.entries.get_mut(key) {
                local_value.merge(other_value);
            } else {
                self.entries.insert(key.clone(), other_value.clone());
            }
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<V: CrdtValue> CrdtValue for MapCrdt<V> {
    type Op = MapOp<V>;
    fn apply(&mut self, op: &Self::Op) {
        self.apply_op(op);
    }
    fn merge(&mut self, other: &Self) {
        MapCrdt::merge(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lww::LwwRegister;

    #[test]
    fn test_put_get_remove() {
        let mut clock = HLC::new(1);
        let mut map: MapCrdt<LwwRegister<String>> = MapCrdt::new(1);

        let val = LwwRegister::new("hello".to_string(), &mut clock);
        map.put("key1".to_string(), val, &mut clock);

        assert!(map.get("key1").is_some());
        assert_eq!(map.get("key1").unwrap().get(), "hello");
        assert_eq!(map.len(), 1);

        map.remove("key1", &mut clock);
        assert!(map.get("key1").is_none());
        assert_eq!(map.len(), 0);
    }
}
