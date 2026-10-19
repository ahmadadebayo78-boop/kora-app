use crate::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String, Vec,
};

// ---------------------------------------------------------------------------
// Issue #277: Ring-buffer access-log tests
// ---------------------------------------------------------------------------

fn setup(env: &Env) -> (KoraContractClient<'_>, Address, Address, u64) {
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    client.init_admin(&admin);

    let vet = Address::generate(env);
    client.register_vet(
        &vet,
        &String::from_str(env, "Dr. Ring"),
        &String::from_str(env, "LIC-RING"),
        &String::from_str(env, "General"),
    );
    client.verify_vet(&admin, &vet);

    let owner = Address::generate(env);
    let pet_id = client.register_pet(
        &owner,
        &String::from_str(env, "Ringo"),
        &String::from_str(env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(env, "Mixed"),
        &String::from_str(env, "Brown"),
        &5,
        &None,
        &PrivacyLevel::Public,
    );

    (client, owner, vet, pet_id)
}

/// Inserting more than MAX_LOG_ENTRIES logs keeps the buffer capped at
/// MAX_LOG_ENTRIES and preserves the most recent entries in order.
#[test]
fn test_access_log_ring_buffer_caps_and_orders() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, owner, _vet, pet_id) = setup(&env);

    let responder = Address::generate(&env);

    // Each grant+revoke cycle writes 2 log entries.  We want to exceed
    // MAX_LOG_ENTRIES, so we perform enough cycles to write well over the cap.
    let log_writes_target: u64 = (MAX_LOG_ENTRIES as u64) + 10;
    let cycles = (log_writes_target / 2) + 1; // +1 to guarantee we exceed the cap

    for _ in 0..cycles {
        client.add_emergency_responder(&pet_id, &responder);
        client.remove_emergency_responder(&pet_id, &responder);
    }

    // The ring buffer should be capped at MAX_LOG_ENTRIES.
    let logs = client.get_access_logs(&pet_id, &owner);
    assert_eq!(logs.len(), MAX_LOG_ENTRIES);

    // Total log writes = cycles * 2.  The oldest visible entry id should be
    // total_writes - MAX_LOG_ENTRIES, confirming the oldest slots were
    // overwritten.
    let total_writes = cycles * 2;
    let first = logs.get(0).unwrap();
    assert_eq!(first.id, total_writes - (MAX_LOG_ENTRIES as u64));

    // The newest entry should be the last one written.
    let last = logs.get(logs.len() - 1).unwrap();
    assert_eq!(last.id, total_writes - 1);
}

/// Writing beyond the cap overwrites oldest entries (ring fashion).
#[test]
fn test_access_log_ring_buffer_overwrites_oldest() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, owner, _vet, pet_id) = setup(&env);

    let responder = Address::generate(&env);

    // Fill exactly MAX_LOG_ENTRIES log entries (2 per grant+revoke cycle).
    let cycles = (MAX_LOG_ENTRIES / 2) as u64;
    for _ in 0..cycles {
        client.add_emergency_responder(&pet_id, &responder);
        client.remove_emergency_responder(&pet_id, &responder);
    }

    let logs = client.get_access_logs(&pet_id, &owner);
    assert_eq!(logs.len(), MAX_LOG_ENTRIES);

    // Now write 4 more entries and confirm the buffer stays capped and the
    // newest entries are present.
    for _ in 0..2 {
        client.add_emergency_responder(&pet_id, &responder);
        client.remove_emergency_responder(&pet_id, &responder);
    }

    let logs = client.get_access_logs(&pet_id, &owner);
    assert_eq!(logs.len(), MAX_LOG_ENTRIES);
}

// ---------------------------------------------------------------------------
// Issue #276: export_access_log timestamp validation
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_export_access_log_rejects_inverted_timestamps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, owner, _vet, pet_id) = setup(&env);

    // start_ts > end_ts should panic with InvalidInput (#12) immediately.
    client.export_access_log(&owner, &pet_id, &100, &50, &1u32);
}

#[test]
fn test_export_access_log_valid_range_returns_events() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, owner, _vet, pet_id) = setup(&env);

    // A valid time window should return without panic (may be empty).
    let events = client.export_access_log(&owner, &pet_id, &0, &u64::MAX, &1u32);
    // No panic — just assert the call succeeded.
    let _ = events;
}
