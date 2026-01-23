//! Physics - physical simulation engine

use crate::wave::{Amplitude, Wave};
use std::collections::HashMap;

/// Physics engine - simulates interactions between waves
pub struct PhysicsEngine {
    /// Wave history (for interference calculations)
    wave_history: HashMap<String, Vec<Wave>>,

    /// Detection threshold for interference patterns
    interference_threshold: f64,
}

impl PhysicsEngine {
    pub fn new() -> Self {
        Self {
            wave_history: HashMap::new(),
            interference_threshold: 0.5,
        }
    }

    /// Calculate interference between two waves
    pub fn calculate_interference(wave1: &Wave, wave2: &Wave) -> Interference {
        let amp1 = wave1.amplitude().value();
        let amp2 = wave2.amplitude().value();

        // Interference pattern by phase difference
        // Same phase -> constructive, opposite phase -> destructive
        let phase_diff =
            (wave1.propagation_count() as f64 - wave2.propagation_count() as f64).abs();

        if phase_diff < 0.5 {
            // Constructive interference
            Interference::Constructive {
                amplitude: Amplitude::new((amp1 + amp2).min(1.0)),
            }
        } else {
            // Destructive interference
            Interference::Destructive {
                amplitude: Amplitude::new((amp1 - amp2).abs()),
            }
        }
    }

    /// Determine whether a wave resonates at a specific channel
    pub fn check_resonance(&self, wave: &Wave, target_frequency: f64) -> Resonance {
        let wave_frequency = self.estimate_frequency(wave);
        let diff = (wave_frequency - target_frequency).abs();

        if diff < 0.1 {
            Resonance::Strong
        } else if diff < 0.3 {
            Resonance::Moderate
        } else {
            Resonance::Weak
        }
    }

    /// Estimate wave frequency (from channel name)
    fn estimate_frequency(&self, wave: &Wave) -> f64 {
        // Simple frequency estimate (use hash of channel name)
        let channel_name = wave.channel().name();
        let hash = channel_name
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        (hash % 1000) as f64 / 1000.0
    }

    /// Detect interference patterns from multiple waves
    pub fn detect_patterns(&mut self, channel: &str, wave: Wave) -> Option<InterferencePattern> {
        let history = self
            .wave_history
            .entry(channel.to_string())
            .or_insert_with(Vec::new);

        // Remove old entries if history grows too large
        if history.len() > 100 {
            history.drain(0..50);
        }

        // Compare the new wave with historical waves
        let mut constructive_count = 0;
        let mut destructive_count = 0;

        for historical_wave in history.iter() {
            match Self::calculate_interference(&wave, historical_wave) {
                Interference::Constructive { .. } => constructive_count += 1,
                Interference::Destructive { .. } => destructive_count += 1,
            }
        }

        history.push(wave);

        let total = constructive_count + destructive_count;
        let threshold = if total == 0 {
            0
        } else {
            (self.interference_threshold * total as f64).ceil() as usize
        };

        if constructive_count > destructive_count
            && constructive_count >= threshold
            && constructive_count > 5
        {
            Some(InterferencePattern::StandingWave)
        } else if destructive_count > constructive_count
            && destructive_count >= threshold
            && destructive_count > 5
        {
            Some(InterferencePattern::Cancellation)
        } else {
            None
        }
    }

    /// Simulate wave diffraction (avoid obstacles)
    pub fn diffract(&self, wave: &mut Wave, obstacle_strength: f64) {
        // Obstacles attenuate amplitude
        let mut amplitude = *wave.amplitude();
        amplitude.attenuate(1.0 - obstacle_strength);
    }
}

impl Default for PhysicsEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Interference types
#[derive(Debug, Clone)]
pub enum Interference {
    /// Constructive interference (amplitude increases)
    Constructive { amplitude: Amplitude },
    /// Destructive interference (amplitude decreases)
    Destructive { amplitude: Amplitude },
}

/// Resonance strength
#[derive(Debug, Clone, PartialEq)]
pub enum Resonance {
    /// Strong resonance
    Strong,
    /// Moderate resonance
    Moderate,
    /// Weak resonance
    Weak,
}

/// Interference patterns
#[derive(Debug, Clone)]
pub enum InterferencePattern {
    /// Standing wave (same pattern repeats)
    StandingWave,
    /// Wave cancellation
    Cancellation,
    /// Complex interference pattern
    Complex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::Channel;

    #[test]
    fn test_physics_engine_creation() {
        let engine = PhysicsEngine::new();
        assert!(engine.wave_history.is_empty());
    }

    #[test]
    fn test_interference_calculation() {
        let wave1 = Wave::builder(Channel::new("test")).amplitude(0.5).build();

        let wave2 = Wave::builder(Channel::new("test")).amplitude(0.5).build();

        let interference = PhysicsEngine::calculate_interference(&wave1, &wave2);

        match interference {
            Interference::Constructive { amplitude } => {
                assert!(amplitude.value() > 0.5);
            }
            _ => panic!("Expected constructive interference"),
        }
    }

    #[test]
    fn test_resonance_check() {
        let engine = PhysicsEngine::new();
        let wave = Wave::builder(Channel::new("test.resonance")).build();

        let resonance = engine.check_resonance(&wave, 0.5);
        assert!(matches!(
            resonance,
            Resonance::Strong | Resonance::Moderate | Resonance::Weak
        ));
    }
}
