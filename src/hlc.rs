use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Hybrid Logical Clock for causal ordering in distributed systems.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct HLC {
    /// Physical time in milliseconds since UNIX epoch.
    pub physical: u64,
    /// Logical counter for ordering within the same physical time.
    pub logical: u64,
    /// Node identifier for deterministic tie-breaking.
    pub node_id: u128,
}

impl HLC {
    pub fn new(node_id: u128) -> Self {
        HLC {
            physical: Self::wall_time(),
            logical: 0,
            node_id,
        }
    }

    /// Issue a new timestamp.
    pub fn now(&mut self) -> HLC {
        let wall = Self::wall_time();
        if wall > self.physical {
            self.physical = wall;
            self.logical = 0;
        } else {
            self.logical += 1;
        }
        self.clone()
    }

    /// Update local clock upon receiving a remote timestamp.
    pub fn receive(&mut self, remote: &HLC) {
        let wall = Self::wall_time();
        if wall > self.physical && wall > remote.physical {
            self.physical = wall;
            self.logical = 0;
        } else if self.physical == remote.physical {
            self.logical = self.logical.max(remote.logical) + 1;
        } else if remote.physical > self.physical {
            self.physical = remote.physical;
            self.logical = remote.logical + 1;
        } else {
            self.logical += 1;
        }
    }

    fn wall_time() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Ord for HLC {
    fn cmp(&self, other: &Self) -> Ordering {
        self.physical
            .cmp(&other.physical)
            .then(self.logical.cmp(&other.logical))
            .then(self.node_id.cmp(&other.node_id))
    }
}

impl PartialOrd for HLC {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monotonic() {
        let mut clock = HLC::new(1);
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t2 > t1);
    }

    #[test]
    fn test_receive_advances() {
        let mut clock_a = HLC::new(1);
        let mut clock_b = HLC::new(2);

        // B is far ahead
        clock_b.physical = clock_a.physical + 10000;
        let remote = clock_b.now();

        clock_a.receive(&remote);
        let local = clock_a.now();
        assert!(local > remote);
    }

    #[test]
    fn test_deterministic_ordering() {
        let a = HLC {
            physical: 100,
            logical: 0,
            node_id: 1,
        };
        let b = HLC {
            physical: 100,
            logical: 0,
            node_id: 2,
        };
        assert!(b > a); // same physical+logical, higher node_id wins
    }
}
