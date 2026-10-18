//! Unit tests for dispute-related validation fixes.
//!
//! Issues covered:
//!   #290 — `submit_evidence`: enforce CID length bounds (32..=128 bytes)
//!   #291 — `set_appeal_window`: enforce lower (86 400 s) and upper (2 592 000 s) bounds
//!   #292 — `raise_dispute`: prevent self-disputes (claimer != target)
//!   #293 — `assign_arbitrator`: require registered arbitrator + ArbitratorAssigned event
use crate::{Gender, KoraContract, KoraContractClient, PrivacyLevel, Species};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String};

// ---------------------------------------------------------------------------
// Shared setup helpers
// ---------------------------------------------------------------------------

/// Registers a minimal pet and returns the `pet_id`.
fn register_pet(env: &Env, client: &KoraContractClient, owner: &Address) -> u64 {
    client.register_pet(
        owner,
        &String::from_str(env, "Rex"),
        &String::from_str(env, "2020-05-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(env, "Black"),
        &String::from_str(env, "German Shepherd"),
        &30u32,
        &None,
        &PrivacyLevel::Public,
    )
}

// ---------------------------------------------------------------------------
// #292 — self-dispute prevention
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_raise_dispute_self_dispute_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let owner = Address::generate(&env);
    let pet_id = register_pet(&env, &client, &owner);

    // claimer == target — must panic with InvalidInput
    client.raise_dispute(
        &pet_id,
        &owner, // claimer
        &owner, // target (same address!)
        &500u64,
        &String::from_str(&env, "Self dispute attempt"),
        &String::from_str(&env, "ipfs://evidence"),
    );
}

#[test]
fn test_raise_dispute_distinct_parties_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let owner = Address::generate(&env);
    let other_party = Address::generate(&env);
    let pet_id = register_pet(&env, &client, &owner);

    // claimer != target — must succeed
    let dispute_id = client.raise_dispute(
        &pet_id,
        &owner,
        &other_party,
        &1_000u64,
        &String::from_str(&env, "Pet not delivered as agreed"),
        &String::from_str(&env, "ipfs://initial-evidence"),
    );

    let dispute = client.get_dispute(&dispute_id).unwrap();
    assert_eq!(dispute.claimer, owner);
    assert_eq!(dispute.target, other_party);
}

// ---------------------------------------------------------------------------
// #290 — submit_evidence CID length bounds
//
// The CID length guard fires before the dispute-phase check, so calling
// submit_evidence on a freshly-raised (Pending) dispute is sufficient to
// trigger the CID validation error.
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_submit_evidence_empty_cid_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let owner = Address::generate(&env);
    let other_party = Address::generate(&env);
    let pet_id = register_pet(&env, &client, &owner);

    let dispute_id = client.raise_dispute(
        &pet_id,
        &owner,
        &other_party,
        &500u64,
        &String::from_str(&env, "Reason"),
        &String::from_str(&env, "ipfs://initial"),
    );

    // CID is empty (len=0) — must panic with InvalidInput before phase check
    client.submit_evidence(
        &dispute_id,
        &other_party,
        &String::from_str(&env, ""),
        &BytesN::from_array(&env, &[0u8; 32]),
    );
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_submit_evidence_short_cid_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let owner = Address::generate(&env);
    let other_party = Address::generate(&env);
    let pet_id = register_pet(&env, &client, &owner);

    let dispute_id = client.raise_dispute(
        &pet_id,
        &owner,
        &other_party,
        &500u64,
        &String::from_str(&env, "Reason"),
        &String::from_str(&env, "ipfs://initial"),
    );

    // 11-character CID — below the 32-byte minimum — must panic
    client.submit_evidence(
        &dispute_id,
        &other_party,
        &String::from_str(&env, "shortcid123"),
        &BytesN::from_array(&env, &[0u8; 32]),
    );
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_submit_evidence_oversized_cid_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let owner = Address::generate(&env);
    let other_party = Address::generate(&env);
    let pet_id = register_pet(&env, &client, &owner);

    let dispute_id = client.raise_dispute(
        &pet_id,
        &owner,
        &other_party,
        &500u64,
        &String::from_str(&env, "Reason"),
        &String::from_str(&env, "ipfs://initial"),
    );

    // Build a 129-character string — above the 128-byte maximum — must panic.
    // We construct it as a Rust std::string::String and convert to soroban String.
    let long_cid: std::string::String = std::iter::repeat('Q').take(129).collect();
    client.submit_evidence(
        &dispute_id,
        &other_party,
        &String::from_str(&env, &long_cid),
        &BytesN::from_array(&env, &[0u8; 32]),
    );
}

// ---------------------------------------------------------------------------
// #293 — assign_arbitrator requires registered arbitrator
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_assign_unregistered_arbitrator_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let random_address = Address::generate(&env);

    // `random_address` was never registered — must panic with InvalidInput
    client.assign_arbitrator(&admin, &random_address);
}

#[test]
fn test_assign_registered_arbitrator_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let arbitrator = Address::generate(&env);

    // First register, then assign — must succeed
    client.register_arbitrator(&admin, &arbitrator);
    client.assign_arbitrator(&admin, &arbitrator);

    // Verify the arbitrator is stored
    let stored = client.get_arbitrator().unwrap();
    assert_eq!(stored, arbitrator);
}

#[test]
fn test_register_arbitrator_by_admin_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let arbitrator = Address::generate(&env);

    // Registering a new address should not panic
    client.register_arbitrator(&admin, &arbitrator);
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_admin_cannot_register_self_as_arbitrator() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // Admin registering themselves — must panic with InvalidInput
    client.register_arbitrator(&admin, &admin);
}

// ---------------------------------------------------------------------------
// #291 — set_appeal_window lower and upper bounds
//
// Tests also exist in test_insurance_appeal.rs per the issue spec; these run
// here because test_insurance_appeal.rs depends on insurance-claim APIs that
// are not yet implemented in lib.rs.
// ---------------------------------------------------------------------------

#[test]
fn test_set_appeal_window_valid_lower_boundary() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // Exactly 1 day (86 400 s) is the minimum valid value — must succeed.
    client.set_appeal_window(&admin, &86_400u64);
}

#[test]
fn test_set_appeal_window_valid_upper_boundary() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // Exactly 30 days (2 592 000 s) is the maximum valid value — must succeed.
    client.set_appeal_window(&admin, &2_592_000u64);
}

#[test]
fn test_set_appeal_window_valid_midrange() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // 7 days — typical value — must succeed.
    client.set_appeal_window(&admin, &(7 * 86_400u64));
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_set_appeal_window_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // 0 seconds — must be rejected.
    client.set_appeal_window(&admin, &0u64);
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_set_appeal_window_below_minimum_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // 86 399 s = 1 day minus 1 second — must be rejected.
    client.set_appeal_window(&admin, &86_399u64);
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_set_appeal_window_above_maximum_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // 2 592 001 s = 30 days + 1 second — must be rejected.
    client.set_appeal_window(&admin, &2_592_001u64);
}

#[test]
#[should_panic(expected = "InvalidInput")]
fn test_set_appeal_window_u64_max_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    // u64::MAX — must be rejected.
    client.set_appeal_window(&admin, &u64::MAX);
}
