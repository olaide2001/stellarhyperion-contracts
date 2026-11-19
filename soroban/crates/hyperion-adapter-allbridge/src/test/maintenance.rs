use super::setup::World;
use crate::storage::{self, DataKey};
use hyperion_core::HyperionError;
use soroban_sdk::{
    testutils::{storage::Persistent, Ledger},
    vec,
};

#[test]
fn permissionless_maintenance_keeps_expiring_peer_mapping_and_admin_unchanged() {
    let w = World::new();
    let chain = w.ethereum();
    let key = DataKey::Lane(chain.clone());
    let before = w.env.as_contract(&w.adapter_id, || {
        let value: crate::types::ChainLane = w.env.storage().persistent().get(&key).unwrap();
        (value, storage::config(&w.env).unwrap().admin)
    });
    w.env
        .ledger()
        .with_mut(|ledger| ledger.sequence_number += storage::BUMP_TO - 10);
    w.env.set_auths(&[]);
    let summary = w.adapter().maintain_storage(
        &vec![&w.env],
        &vec![&w.env, chain.clone()],
        &vec![&w.env],
        &vec![&w.env],
        &vec![&w.env],
    );
    assert_eq!(summary.peer_trust_touched, 2);
    assert_eq!(summary.total_keys_touched, 2);
    w.env
        .ledger()
        .with_mut(|ledger| ledger.sequence_number += 20);
    w.env.as_contract(&w.adapter_id, || {
        let value: crate::types::ChainLane = w.env.storage().persistent().get(&key).unwrap();
        assert_eq!(value, before.0);
        assert_eq!(storage::config(&w.env).unwrap().admin, before.1);
        assert!(w.env.storage().persistent().get_ttl(&key) > storage::BUMP_TO - 30);
    });
}

#[test]
fn combined_physical_peer_key_footprint_is_bounded() {
    let w = World::new();
    let mut chains = vec![&w.env];
    for _ in 0..17 {
        chains.push_back(w.ethereum());
    }
    assert_eq!(
        w.adapter().try_maintain_storage(
            &vec![&w.env],
            &chains,
            &vec![&w.env],
            &vec![&w.env],
            &vec![&w.env]
        ),
        Err(Ok(HyperionError::InvalidLimit))
    );
}
