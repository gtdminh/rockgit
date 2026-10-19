use super::*;

#[test]
fn provider_lookup_round_trips() {
    for (index, (provider, _)) in PROVIDER_OPTIONS.iter().enumerate() {
        assert_eq!(provider_for_index(index), Some(*provider));
        assert_eq!(provider_index(*provider), index);
    }
}
