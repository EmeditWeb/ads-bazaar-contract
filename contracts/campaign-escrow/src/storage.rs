#![allow(dead_code)]

use ads_bazaar_shared::CampaignId;
use soroban_sdk::{contracttype, Address, Env, String, Vec};

use crate::error::Error;
use crate::types::{Application, Campaign};

/// Extend persistent entries by roughly this many ledgers on every write
/// (~30 days at 5s/ledger). TODO(contributors): tune once real rent/TTL
/// costs on target networks are benchmarked, and consider a max-TTL bump on
/// read-heavy paths too.
const PERSISTENT_BUMP_LEDGERS: u32 = 518_400;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = 500_000;

/// Same ~30-day-at-5s/ledger bump as `PERSISTENT_BUMP_LEDGERS`, but for
/// instance storage (admin/treasury/fee_bps/dispute_contract config keys).
/// Kept as a separate constant since instance and persistent TTL are
/// tracked independently by the ledger even when the numbers happen to match.
const INSTANCE_BUMP_LEDGERS: u32 = 518_400;
const INSTANCE_LIFETIME_THRESHOLD: u32 = 500_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    PendingAdmin,
    Treasury,
    FeeBps,
    DisputeContract,
    Version,
    NextCampaignId,
    Campaign(CampaignId),
    Application(CampaignId, Address),
    /// Whether the contract is currently paused. See `require_not_paused`
    /// and `pause`/`unpause` in `lib.rs`.
    Paused,
    /// Ordered list of creator addresses that have applied to a campaign.
    /// Used by `campaign_applicants` to enumerate all applicants so clients
    /// can list them without relying on event logs alone. Also used by
    /// `update_campaign_metadata` to enforce that the brief is locked once
    /// any creator has applied.
    CampaignApplicants(CampaignId),
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

/// Bump the instance entry's TTL. Call this from any read-heavy path that
/// touches instance storage (config reads, not just writes) so the config
/// doesn't expire from lack of writes alone.
pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_LEDGERS);
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

pub fn set_pending_admin(env: &Env, pending_admin: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::PendingAdmin, pending_admin);
}

pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
}

pub fn set_treasury(env: &Env, treasury: &Address) {
    env.storage().instance().set(&DataKey::Treasury, treasury);
}

pub fn get_treasury(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Treasury)
        .ok_or(Error::NotInitialized)
}

pub fn set_fee_bps(env: &Env, fee_bps: i128) {
    env.storage().instance().set(&DataKey::FeeBps, &fee_bps);
}

pub fn get_fee_bps(env: &Env) -> Result<i128, Error> {
    env.storage()
        .instance()
        .get(&DataKey::FeeBps)
        .ok_or(Error::NotInitialized)
}

pub fn set_dispute_contract(env: &Env, dispute_contract: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::DisputeContract, dispute_contract);
}

pub fn get_dispute_contract(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::DisputeContract)
        .ok_or(Error::NotInitialized)
}

pub fn set_version(env: &Env, version: &String) {
    env.storage().instance().set(&DataKey::Version, version);
}

pub fn get_version(env: &Env) -> Result<String, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Version)
        .ok_or(Error::NotInitialized)
}

pub fn next_campaign_id(env: &Env) -> CampaignId {
    let id: CampaignId = env
        .storage()
        .instance()
        .get(&DataKey::NextCampaignId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::NextCampaignId, &(id + 1));
    id
}

pub fn get_campaign(env: &Env, id: CampaignId) -> Result<Campaign, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Campaign(id))
        .ok_or(Error::CampaignNotFound)
}

pub fn set_campaign(env: &Env, campaign: &Campaign) {
    let key = DataKey::Campaign(campaign.id);
    env.storage().persistent().set(&key, campaign);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_LEDGERS,
    );
}

pub fn get_application(
    env: &Env,
    campaign_id: CampaignId,
    creator: &Address,
) -> Result<Application, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Application(campaign_id, creator.clone()))
        .ok_or(Error::ApplicationNotFound)
}

pub fn set_application(env: &Env, application: &Application) {
    let key = DataKey::Application(application.campaign_id, application.creator.clone());
    env.storage().persistent().set(&key, application);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_LEDGERS,
    );
}

/// Read the current pause state. Defaults to `false` (unpaused) if never
/// explicitly set, which is also the correct behavior pre-`initialize`.
pub fn get_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

/// Append the creator's address to the ordered applicant list for
/// `campaign_id`. Called from `apply_to_campaign` so clients can enumerate
/// every applicant via `campaign_applicants` without relying on event
/// logs alone, and so `update_campaign_metadata` can lock the brief once
/// at least one creator has applied.
pub fn add_campaign_applicant(env: &Env, campaign_id: CampaignId, creator: &Address) {
    let key = DataKey::CampaignApplicants(campaign_id);
    let mut applicants: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    applicants.push_back(creator.clone());
    env.storage().persistent().set(&key, &applicants);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_LEDGERS,
    );
}

/// Return whether any creator has applied to `campaign_id`.
pub fn has_campaign_applicants(env: &Env, campaign_id: CampaignId) -> bool {
    let applicants: Option<Vec<Address>> = env
        .storage()
        .persistent()
        .get(&DataKey::CampaignApplicants(campaign_id));
    applicants.is_some_and(|v| !v.is_empty())
}

/// Return the ordered list of creator addresses that have applied to
/// `campaign_id`. Returns an empty `Vec` when no one has applied yet.
/// Bumps the TTL of the `CampaignApplicants` persistent entry on every
/// read so it doesn't expire from ledger storage (only when the entry
/// already exists — an empty list with no stored key is harmless to
/// skip).
pub fn get_campaign_applicants(env: &Env, campaign_id: CampaignId) -> Vec<Address> {
    let key = DataKey::CampaignApplicants(campaign_id);
    let applicants: Option<Vec<Address>> = env.storage().persistent().get(&key);
    if let Some(ref _applicants) = applicants {
        env.storage().persistent().extend_ttl(
            &key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
    }
    applicants.unwrap_or_else(|| Vec::new(env))
}
