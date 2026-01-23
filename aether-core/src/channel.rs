//! Channel - frequency space for waves

use serde::{Deserialize, Serialize};
use std::fmt;

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
}
