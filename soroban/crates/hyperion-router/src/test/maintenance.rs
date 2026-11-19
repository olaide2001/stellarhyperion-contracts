use super::{
    doubles::{Delivery, FailableToken, FailableTokenClient},
    setup::{World, HUNDRED},
};
use crate::{
    storage, ActionAuditStatus, AdminAction, CancelReason, DataKey, HyperionRouter,
    HyperionRouterClient, PendingClaim, TransferRecord, CURRENT_SCHEMA_VERSION, MIN_TIMELOCK_DELAY,
};
use hyperion_core::{AddressKind, HyperionError, RouteKind};
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, Address, BytesN, IntoVal, Map, String, Symbol, Val,
};

#[test]
fn cancellation_keeps_payload_and_rejects_unsafe_text_without_cancelling() {
    let w = World::bare();
    let action = AdminAction::SetFeeBps(22);
    let queued = w.router().queue_action(&w.admin, &action);
    for note in [
        String::from_str(&w.env, "bad\nline"),
        String::from_str(&w.env, "é"),
        String::from_str(&w.env, &"a".repeat(129)),
    ] {
        assert_eq!(
            w.router().try_cancel_action_with_reason(
                &w.guardian,
                &queued,
                &CancelReason::SecurityConcern,
                &note
            ),
            Err(Ok(HyperionError::InvalidLimit))
        );
        assert_eq!(w.router().get_queued(&queued).action, action);
    }
    w.router().cancel_action_with_reason(
        &w.guardian,
        &queued,
        &CancelReason::SecurityConcern,
        &String::from_str(&w.env, "review needed"),
    );
    let events = w.env.events().all();
    let soroban_sdk::xdr::ContractEventBody::V0(body) = &events.events().last().unwrap().body;
    let data: Val = body.data.clone().into_val(&w.env);
    let event: Map<Symbol, Val> = data.into_val(&w.env);
    let version: u32 = event
        .get(Symbol::new(&w.env, "version"))
        .unwrap()
        .into_val(&w.env);
    let actor: Address = event
        .get(Symbol::new(&w.env, "actor"))
        .unwrap()
        .into_val(&w.env);
    let status: ActionAuditStatus = event
        .get(Symbol::new(&w.env, "status"))
        .unwrap()
        .into_val(&w.env);
    assert_eq!(version, 1);
    assert_eq!(actor, w.guardian);
    assert_eq!(status, ActionAuditStatus::Cancelled);
    assert_eq!(w.router().get_action_payload(&queued).action, action);
    w.advance_time(MIN_TIMELOCK_DELAY + 1);
    assert_eq!(
        w.router().try_execute_action(&w.admin, &queued),
        Err(Ok(HyperionError::TimelockNotQueued))
    );
}

#[test]
fn legacy_cancellation_preserves_payload_without_inventing_a_queuing_actor() {
    let w = World::bare();
    let action = AdminAction::SetFeeBps(22);
    let queued = 99u64;
    let old = crate::QueuedAction {
        id: queued,
        action: action.clone(),
        queued_at: 1700000000,
        eta: 1700000000 + MIN_TIMELOCK_DELAY,
        expires_at: 1700000000 + MIN_TIMELOCK_DELAY + crate::GRACE_PERIOD,
    };
    // Seed the old wire shape directly; there is no queuing audit from which to infer an author.
    w.env.as_contract(&w.router_id, || {
        w.env
            .storage()
            .persistent()
            .set(&DataKey::Queued(queued), &old)
    });
    w.router().cancel_action(&w.guardian, &queued);
    let events = w.env.events().all();
    let soroban_sdk::xdr::ContractEventBody::V0(body) = &events.events().last().unwrap().body;
    let data: Val = body.data.clone().into_val(&w.env);
    let event: Map<Symbol, Val> = data.into_val(&w.env);
    let version: u32 = event
        .get(Symbol::new(&w.env, "version"))
        .unwrap()
        .into_val(&w.env);
    let actor: Address = event
        .get(Symbol::new(&w.env, "actor"))
        .unwrap()
        .into_val(&w.env);
    let status: ActionAuditStatus = event
        .get(Symbol::new(&w.env, "status"))
        .unwrap()
        .into_val(&w.env);
    assert_eq!(version, 1);
    assert_eq!(actor, w.guardian);
    assert_eq!(status, ActionAuditStatus::Cancelled);
    assert_eq!(w.router().get_action_payload(&queued).action, action);
}

fn legacy_records(w: &World) -> (TransferRecord, PendingClaim) {
    let transfer = TransferRecord {
        nonce: 1,
        sender: w.user.clone(),
        token: w.token_id.clone(),
        gross_amount: HUNDRED + 7,
        fee: 12345,
        net_amount: HUNDRED - 12338,
        destination_chain: w.ethereum(),
        destination: BytesN::from_array(&w.env, &[3; 32]),
        route: RouteKind::Cctp,
        created_ledger: 100000,
        created_at: 1700000000,
    };
    let claim = PendingClaim {
        id: 1,
        recipient: w.recipient.clone(),
        token: w.token_id.clone(),
        amount: HUNDRED + 11,
        route: RouteKind::AxelarIts,
        source_chain: w.ethereum(),
        source_nonce: 99,
        created_at: 1700000000,
        settled: false,
    };
    w.env.as_contract(&w.router_id, || {
        w.env.storage().instance().remove(&DataKey::SchemaVersion);
        w.env.storage().instance().set(&DataKey::OutNonce, &1u64);
        w.env.storage().instance().set(&DataKey::ClaimSeq, &1u64);
        w.env
            .storage()
            .persistent()
            .set(&DataKey::Transfer(1), &transfer);
        w.env.storage().persistent().set(&DataKey::Claim(1), &claim);
    });
    (transfer, claim)
}

#[test]
fn migration_requires_review_is_interruptible_preserves_exact_values_and_has_fixed_totals() {
    let w = World::bare();
    let (transfer, claim) = legacy_records(&w);
    assert_eq!(w.router().get_migration_status().current_version, 1);
    assert_eq!(
        w.router().try_migrate_storage(&w.admin, &2, &1),
        Err(Ok(HyperionError::TimelockNotQueued))
    );
    let q = w
        .router()
        .queue_action(&w.admin, &AdminAction::BeginStorageMigration(2));
    assert_eq!(
        w.router().try_execute_action(&w.admin, &q),
        Err(Ok(HyperionError::TimelockNotReady))
    );
    w.advance_time(MIN_TIMELOCK_DELAY + 1);
    w.router().execute_action(&w.admin, &q);
    assert_eq!(
        w.router().try_migrate_storage(&w.guardian, &2, &1),
        Err(Ok(HyperionError::Unauthorized))
    );
    let first = w.router().migrate_storage(&w.admin, &2, &1);
    assert!(first.migration_pending);
    assert_eq!(first.migrated_transfers, 1);
    assert_eq!(w.router().get_transfer(&1), transfer);
    assert_eq!(w.router().get_claim(&1), claim);
    assert_eq!(first.current_version, 1);
    assert_eq!(first.target_version, 2);
    assert_eq!(first.migrated_claims, 0);
    w.env.as_contract(&w.router_id, || {
        w.env.storage().instance().set(&DataKey::OutNonce, &2u64);
        let mut new = transfer.clone();
        new.nonce = 2;
        storage::set_transfer(&w.env, &new);
    });
    let done = w.router().migrate_storage(&w.admin, &2, &1);
    assert!(!done.migration_pending);
    assert_eq!(done.total_transfers, 1);
    assert_eq!(done.current_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(w.router().get_claim(&1), claim);
    assert_eq!(w.router().migrate_storage(&w.admin, &2, &1), done);
}

#[test]
fn migration_refuses_missing_records_instead_of_marking_them_migrated() {
    let w = World::bare();
    legacy_records(&w);
    let q = w
        .router()
        .queue_action(&w.admin, &AdminAction::BeginStorageMigration(2));
    w.advance_time(MIN_TIMELOCK_DELAY + 1);
    w.router().execute_action(&w.admin, &q);
    w.env.as_contract(&w.router_id, || {
        w.env.storage().persistent().remove(&DataKey::Claim(1))
    });
    assert_eq!(
        w.router().try_migrate_storage(&w.admin, &2, &2),
        Err(Ok(HyperionError::ClaimNotFound))
    );
    assert_eq!(w.router().get_migration_status().migrated_transfers, 0);
    assert_eq!(w.router().get_migration_status().current_version, 1);
}

#[test]
fn combined_upkeep_footprints_are_bounded() {
    let w = World::bare();
    let mut claims = vec![&w.env];
    for id in 1..=33 {
        claims.push_back(id);
    }
    assert_eq!(
        w.router()
            .try_keep_alive(&vec![&w.env], &vec![&w.env], &claims, &vec![&w.env]),
        Err(Ok(soroban_sdk::Error::from_contract_error(
            HyperionError::InvalidLimit as u32
        )))
    );
}

#[test]
fn archived_snapshot_restores_only_recorded_claim_and_never_redirects_or_repeats_payment() {
    let w = World::new();
    let token = w.with_failable_token();
    w.failable(&token).mint(&w.rail_id, &HUNDRED);
    w.failable(&token).set_blocked(&w.recipient, &true);
    let id = w.rail().deliver(&Delivery {
        route: RouteKind::Cctp,
        token: token.clone(),
        amount: HUNDRED,
        recipient: w.recipient.clone(),
        recipient_kind: AddressKind::Account,
        source_chain: w.ethereum(),
        source_nonce: 41,
    });
    w.failable(&token).set_blocked(&w.recipient, &false);
    let mut snapshot = w.env.to_snapshot();
    use soroban_sdk::{
        xdr::{LedgerKey, ScVal},
        IntoVal, Val,
    };
    let key: ScVal = w.env.as_contract(&w.router_id, || {
        let val: Val = DataKey::Claim(id).into_val(&w.env);
        val.into_val(&w.env)
    });
    let current = snapshot.ledger.sequence_number;
    let entry = snapshot
        .ledger
        .ledger_entries
        .iter_mut()
        .find(|(k, _)| matches!(k.as_ref(), LedgerKey::ContractData(data) if data.key == key))
        .unwrap();
    entry.1 .1 = Some(current - 1); // Genuine archived persistent-entry TTL in the fixture.
    let mut restored = snapshot.clone();
    let entry = restored
        .ledger
        .ledger_entries
        .iter_mut()
        .find(|(k, _)| matches!(k.as_ref(), LedgerKey::ContractData(data) if data.key == key))
        .unwrap();
    entry.1 .1 = Some(current + storage::BUMP_TO); // Fixture application of RestoreFootprintOp, payload unchanged.
    let env = soroban_sdk::Env::from_snapshot(restored);
    fn rebound(env: &soroban_sdk::Env, address: &Address) -> Address {
        let mut raw = [0u8; 56];
        address.to_string().copy_into_slice(&mut raw);
        Address::from_string(&String::from_str(env, core::str::from_utf8(&raw).unwrap()))
    }
    let router_id = rebound(&env, &w.router_id);
    let token = rebound(&env, &token);
    let recipient = rebound(&env, &w.recipient);
    env.register_at(&router_id, HyperionRouter, ());
    env.register_at(&token, FailableToken, ());
    env.mock_all_auths();
    let client = HyperionRouterClient::new(&env, &router_id);
    let coins = FailableTokenClient::new(&env, &token);
    let attacker = Address::generate(&env);
    assert_eq!(client.get_claim(&id).recipient, recipient);
    client.settle_claim(&attacker, &id);
    assert_eq!(coins.balance(&recipient), HUNDRED);
    assert_eq!(coins.balance(&attacker), 0);
    assert_eq!(coins.balance(&router_id), 0);
    assert_eq!(
        client.try_settle_claim(&attacker, &id),
        Err(Ok(HyperionError::ClaimAlreadySettled))
    );
}
