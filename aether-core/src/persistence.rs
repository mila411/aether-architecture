//! Persistence: append-only log and snapshot for restart recovery.

use crate::{AetherStats, Wave};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::path::Path;

const META_TREE: &str = "meta";
const LOG_TREE: &str = "log";
const KEY_LAST_INDEX: &[u8] = b"last_index";
const KEY_SNAPSHOT: &[u8] = b"snapshot";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AetherSnapshot {
    pub last_index: u64,
    pub stats: AetherStats,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WaveStore {
    db: Db,
    log: Tree,
    meta: Tree,
}

impl WaveStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = sled::open(path)?;
        let log = db.open_tree(LOG_TREE)?;
        let meta = db.open_tree(META_TREE)?;
        Ok(Self { db, log, meta })
    }

    pub fn append_wave(&self, wave: &Wave) -> Result<u64> {
        let index = self.next_index()?;
        let key = index.to_be_bytes();
        let value = serde_json::to_vec(wave)?;
        self.log.insert(key, value)?;
        self.meta.insert(KEY_LAST_INDEX, index.to_be_bytes().as_slice())?;
        Ok(index)
    }

    pub fn load_snapshot(&self) -> Result<Option<AetherSnapshot>> {
        match self.meta.get(KEY_SNAPSHOT)? {
            Some(bytes) => {
                let snapshot = serde_json::from_slice::<AetherSnapshot>(&bytes)?;
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    pub fn save_snapshot(&self, snapshot: &AetherSnapshot) -> Result<()> {
        let bytes = serde_json::to_vec(snapshot)?;
        self.meta.insert(KEY_SNAPSHOT, bytes)?;
        Ok(())
    }

    pub fn read_from(&self, start_index: u64) -> Result<Vec<Wave>> {
        let mut waves = Vec::new();
        for item in self
            .log
            .range(start_index.to_be_bytes()..)
        {
            let (_, value) = item?;
            let wave = serde_json::from_slice::<Wave>(&value)?;
            waves.push(wave);
        }
        Ok(waves)
    }

    fn next_index(&self) -> Result<u64> {
        if let Some(bytes) = self.meta.get(KEY_LAST_INDEX)? {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes);
            Ok(u64::from_be_bytes(arr) + 1)
        } else {
            Ok(0)
        }
    }

    pub fn flush(&self) -> Result<()> {
        self.db.flush()?;
        Ok(())
    }
}
