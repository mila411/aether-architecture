//! Channel - frequency space for waves

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// A channel represents a specific frequency band and acts as a message category
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Channel {
    /// Channel name (e.g., "orders", "payments", "notifications")
    name: String,
    /// Supports hierarchical structure (e.g., "orders.created", "orders.updated")
    segments: Vec<String>,
}

impl Channel {
    /// Create a new channel
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let segments = name.split('.').map(|s| s.to_string()).collect();

        Self { name, segments }
    }

    /// Get the channel name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Determine whether this channel matches another channel
    /// Supports wildcard ("*")
    pub fn matches(&self, pattern: &Channel) -> bool {
        if pattern.name == "*" {
            return true;
        }

        if self.segments.len() != pattern.segments.len() {
            // Support patterns like "orders.*"
            if pattern.segments.last() == Some(&"*".to_string()) {
                let pattern_prefix = &pattern.segments[..pattern.segments.len() - 1];
                return self.segments.starts_with(pattern_prefix);
            }
            return false;
        }

        self.segments
            .iter()
            .zip(pattern.segments.iter())
            .all(|(s, p)| p == "*" || s == p)
    }

    /// Create a child channel by concatenation
    pub fn child(&self, child_name: &str) -> Self {
        Self::new(format!("{}.{}", self.name, child_name))
    }

    /// Create a frequency-hopped channel derived from this channel
    pub fn hop(&self, hop_index: u16, hop_count: u16) -> Self {
        let count = hop_count.max(1);
        let idx = hop_index % count;
        Self::new(format!("{}.hop{}", self.name, idx))
    }

    /// Create a set of frequency-hopped channels derived from this channel
    pub fn hop_set(&self, hop_count: u16) -> Vec<Channel> {
        let count = hop_count.max(1);
        (0..count).map(|idx| self.hop(idx, count)).collect()
    }

    /// Compute hop index at a given timestamp (milliseconds since epoch)
    pub fn hop_index_at_ms(&self, timestamp_ms: u64, hop_count: u16, hop_interval_ms: u64) -> u16 {
        let count = hop_count.max(1) as u64;
        let interval = hop_interval_ms.max(1);
        let slot = timestamp_ms / interval;
        let seed = self.hop_seed();
        ((slot.wrapping_add(seed)) % count) as u16
    }

    /// Create a frequency-hopped channel at a given timestamp (milliseconds since epoch)
    pub fn hop_at_ms(&self, timestamp_ms: u64, hop_count: u16, hop_interval_ms: u64) -> Self {
        let idx = self.hop_index_at_ms(timestamp_ms, hop_count, hop_interval_ms);
        self.hop(idx, hop_count)
    }

    /// Create a frequency-hopped channel based on current system time
    pub fn hop_now(&self, hop_count: u16, hop_interval_ms: u64) -> Self {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.hop_at_ms(now_ms, hop_count, hop_interval_ms)
    }

    fn hop_seed(&self) -> u64 {
        self.name
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(131).wrapping_add(b as u64))
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<String> for Channel {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl From<&str> for Channel {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_creation() {
        let channel = Channel::new("orders.created");
        assert_eq!(channel.name(), "orders.created");
    }

    #[test]
    fn test_channel_matching() {
        let channel = Channel::new("orders.created");
        let pattern1 = Channel::new("orders.created");
        let pattern2 = Channel::new("orders.*");
        let pattern3 = Channel::new("*");
        let pattern4 = Channel::new("payments.created");

        assert!(channel.matches(&pattern1));
        assert!(channel.matches(&pattern2));
        assert!(channel.matches(&pattern3));
        assert!(!channel.matches(&pattern4));
    }

    #[test]
    fn test_channel_child() {
        let parent = Channel::new("orders");
        let child = parent.child("created");
        assert_eq!(child.name(), "orders.created");
    }

    #[test]
    fn test_channel_hop() {
        let base = Channel::new("orders");
        let hop = base.hop(2, 5);
        assert_eq!(hop.name(), "orders.hop2");
    }

    #[test]
    fn test_channel_hop_set() {
        let base = Channel::new("orders");
        let hops = base.hop_set(3);
        assert_eq!(hops.len(), 3);
        assert_eq!(hops[0].name(), "orders.hop0");
        assert_eq!(hops[2].name(), "orders.hop2");
    }

    #[test]
    fn test_channel_hop_at_ms_is_stable() {
        let base = Channel::new("orders");
        let hop1 = base.hop_at_ms(1_000, 5, 200);
        let hop2 = base.hop_at_ms(1_000, 5, 200);
        assert_eq!(hop1.name(), hop2.name());
    }
}
