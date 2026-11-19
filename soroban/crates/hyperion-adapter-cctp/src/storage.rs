use hyperion_core::HyperionError;
use soroban_sdk::{contracttype, Address, BytesN, Env, String};

use crate::types::Config;

pub const BUMP_THRESHOLD: u32 = 17_280;
pub const BUMP_TO: u32 = 518_400;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    /// Chain name to Circle domain, for the outbound leg.
    Domain(String),
    /// Circle domain back to chain name, for the inbound leg. Two keys rather than one scan,
    /// because a contract cannot iterate storage and both directions are on a hot path.
    Chain(u32),
    /// Per destination, who is allowed to broadcast the message over there.
    Caller(u32),
    /// `(source domain, the remote token that was burned)` to the local asset Circle mints.
    ///
    /// This doubles as the inbound allowlist. An unmapped pair is refused, which is the correct
    /// behaviour on the day Circle adds a token pair nobody here has looked at yet.
    Asset(u32, BytesN<32>),
}

pub fn config(env: &Env) -> Result<Config, HyperionError> {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(HyperionError::NotInitialized)
}

pub fn set_config(env: &Env, cfg: &Config) {
    env.storage().instance().set(&DataKey::Config, cfg);
    env.storage().instance().extend_ttl(BUMP_THRESHOLD, BUMP_TO);
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Config)
}

pub fn domain_of(env: &Env, chain: &String) -> Option<u32> {
    let key = DataKey::Domain(chain.clone());
    let found: Option<u32> = env.storage().persistent().get(&key);
    if found.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
    }
    found
}

pub fn chain_of(env: &Env, domain: u32) -> Option<String> {
    let key = DataKey::Chain(domain);
    let found: Option<String> = env.storage().persistent().get(&key);
    if found.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
    }
    found
}

pub fn destination_caller(env: &Env, domain: u32) -> BytesN<32> {
    env.storage()
        .persistent()
        .get(&DataKey::Caller(domain))
        .unwrap_or_else(|| BytesN::from_array(env, &[0u8; 32]))
}

pub fn set_link(env: &Env, chain: &String, domain: u32, caller: &BytesN<32>) {
    let domain_key = DataKey::Domain(chain.clone());
    let chain_key = DataKey::Chain(domain);
    let caller_key = DataKey::Caller(domain);
    env.storage().persistent().set(&domain_key, &domain);
    env.storage().persistent().set(&chain_key, chain);
    env.storage().persistent().set(&caller_key, caller);
    env.storage()
        .persistent()
        .extend_ttl(&domain_key, BUMP_THRESHOLD, BUMP_TO);
    env.storage()
        .persistent()
        .extend_ttl(&chain_key, BUMP_THRESHOLD, BUMP_TO);
    env.storage()
        .persistent()
        .extend_ttl(&caller_key, BUMP_THRESHOLD, BUMP_TO);
}

pub fn asset(env: &Env, domain: u32, burn_token: &BytesN<32>) -> Option<Address> {
    let key = DataKey::Asset(domain, burn_token.clone());
    let found: Option<Address> = env.storage().persistent().get(&key);
    if found.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
    }
    found
}

pub fn set_asset(env: &Env, domain: u32, burn_token: &BytesN<32>, local: &Address) {
    let key = DataKey::Asset(domain, burn_token.clone());
    env.storage().persistent().set(&key, local);
    env.storage()
        .persistent()
        .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
}

fn touch_key(env: &Env, key: &DataKey) -> bool {
    if env.storage().persistent().has(key) {
        env.storage()
            .persistent()
            .extend_ttl(key, BUMP_THRESHOLD, BUMP_TO);
        true
    } else {
        false
    }
}

/// Top a link's lifetime back up without changing it.
///
/// Soroban archives storage nobody reads, and a lane that stays quiet for a month is exactly the
/// lane somebody will try to use on the day it has expired. The read paths above bump as they go,
/// so this only matters for a lane with no traffic at all, which is why it is permissionless.
pub fn touch_link(env: &Env, domain: u32) -> u32 {
    let mut touched = 0u32;
    let chain_key = DataKey::Chain(domain);
    if let Some(chain) = env.storage().persistent().get::<_, String>(&chain_key) {
        env.storage()
            .persistent()
            .extend_ttl(&chain_key, BUMP_THRESHOLD, BUMP_TO);
        touched += 1;
        if touch_key(env, &DataKey::Domain(chain)) {
            touched += 1;
        }
    }
    if touch_key(env, &DataKey::Caller(domain)) {
        touched += 1;
    }
    touched
}

pub fn touch_link_by_chain(env: &Env, chain: &String) -> u32 {
    let domain_key = DataKey::Domain(chain.clone());
    let Some(domain) = env.storage().persistent().get::<_, u32>(&domain_key) else {
        return 0;
    };
    env.storage()
        .persistent()
        .extend_ttl(&domain_key, BUMP_THRESHOLD, BUMP_TO);
    let mut touched = 1u32;
    if touch_key(env, &DataKey::Chain(domain)) {
        touched += 1;
    }
    if touch_key(env, &DataKey::Caller(domain)) {
        touched += 1;
    }
    touched
}

pub fn touch_asset(env: &Env, domain: u32, burn_token: &BytesN<32>) -> bool {
    touch_key(env, &DataKey::Asset(domain, burn_token.clone()))
}
