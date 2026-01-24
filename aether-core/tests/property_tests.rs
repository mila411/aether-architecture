use aether_core::{Channel, Wave};
use proptest::prelude::*;

proptest! {
    #[test]
    fn channel_matching_symmetry(base in "[a-z]{1,6}") {
        let channel = Channel::new(format!("{}.created", base));
        let pattern = Channel::new(format!("{}.*", base));
        prop_assert!(channel.matches(&pattern));
    }

    #[test]
    fn wave_schema_is_compatible(payload in "[a-zA-Z0-9]{0,32}") {
        let wave = Wave::new("test.channel", serde_json::json!({"payload": payload}));
        prop_assert!(wave.is_compatible());
    }
}
