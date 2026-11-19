//! Shared storage maintenance interfaces and bounded upkeep summaries.
//!
//! Soroban persistent entries expire unless their TTL is extended. Both the router and its rail
//! adapters expose permissionless, bounded maintenance entrypoints using these shared types so
//! keepers and operators can verify explicit key coverage across token mappings, peer trust
//! links, and pending/processed inbound records.

use soroban_sdk::{contractclient, contracttype, Address, BytesN, Env, String, Vec};

use crate::error::HyperionError;
use crate::route::RouteKind;

/// Hard upper bound on each input vector in a single storage maintenance or restoration call.
///
/// Keeping batches bounded prevents a single call from exceeding Soroban's instruction or
/// footprint limits.
pub const MAX_MAINTENANCE_BATCH: u32 = 32;

/// Counts of active storage keys whose TTL was extended in a single maintenance call.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaintenanceSummary {
    pub instance_extended: bool,
    pub token_mappings_touched: u32,
    pub peer_trust_touched: u32,
    pub inbound_records_touched: u32,
    pub total_keys_touched: u32,
}

/// Explicit key coverage metadata for a router or adapter contract.
///
/// Documents which persistent key families exist on a given contract and how many distinct
/// key shapes are covered by its bounded maintenance entrypoint.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterKeyCoverage {
    pub route: RouteKind,
    pub has_token_mappings: bool,
    pub has_peer_trust: bool,
    pub has_inbound_records: bool,
    pub token_mapping_keys: u32,
    pub peer_trust_keys: u32,
    pub inbound_record_keys: u32,
}

/// Bounded storage maintenance interface implemented by rail adapters.
#[contractclient(name = "AdapterMaintenanceClient")]
pub trait AdapterMaintenance {
    fn key_coverage(env: Env) -> Result<AdapterKeyCoverage, HyperionError>;
    /// Extend TTL on bounded batches of adapter persistent entries.
    fn maintain_storage(
        env: Env,
        tokens: Vec<Address>,
        peer_chains: Vec<String>,
        peer_domains: Vec<u32>,
        remote_tokens: Vec<BytesN<32>>,
        inbound_messages: Vec<BytesN<32>>,
    ) -> Result<MaintenanceSummary, HyperionError>;
}
