/// Tests for GitHub issues #198, #199, #200, #201
///
/// #198 – activate_pet and deactivate_pet must panic with ContractError::PetNotFound
///         when the pet does not exist, instead of silently returning.
/// #199 – activate_pet must panic with ContractError::PetIsArchived when the pet
///         is currently archived, enforcing the lifecycle invariant that archived
///         pets must be explicitly unarchived before they can be activated.
/// #200 – archive_pet must emit PetArchivedEvent and unarchive_pet must emit
///         PetUnarchivedEvent so that off-chain indexers can track state changes.
/// #201 – add_pet_photo must reject duplicate photo hashes with
///         ContractError::DuplicateEntry and reject a 51st photo with
///         ContractError::StorageQuotaExceeded.

use soroban_sdk::{testutils::Address as _, Address, Env, String, TryFromVal};

use crate::{Gender, KoraContract, KoraContractClient, PrivacyLevel, Species};

// ---------------------------------------------------------------------------
// Shared setup helpers
// ---------------------------------------------------------------------------

fn setup_env() -> (Env, KoraContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, KoraContract);
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    (env, client, admin)
}

fn register_pet(client: &KoraContractClient, env: &Env, owner: &Address) -> u64 {
    client.register_pet(
        owner,
        &String::from_str(env, "Buddy"),
        &String::from_str(env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(env, "Labrador"),
        &String::from_str(env, "Brown"),
        &25u32,
        &None,
        &PrivacyLevel::Public,
    )
}

/// Return a valid CIDv0 hash that is unique per `index`.
/// CIDv0 format: exactly 46 characters, starting with "Qm", all base58.
/// We encode `index` into the last 4 positions so each 0..58^4 value is unique.
fn photo_hash_for_index(env: &Env, index: u32) -> String {
    // Base58 alphabet (no 0, O, I, l).
    const B58: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    // Build a 46-char buffer filled with valid base58 characters.
    let mut buf = [b'Y'; 46];
    buf[0] = b'Q';
    buf[1] = b'm';
    // Positions 2..42 are fixed valid base58 characters (exactly 40 bytes).
    let fixed: &[u8; 40] = b"wAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnP";
    buf[2..42].copy_from_slice(fixed);
    // Positions 42..46 encode `index` in base58 — gives 58^4 = ~11M unique values.
    let mut n = index;
    for slot in (42..46).rev() {
        buf[slot] = B58[(n % 58) as usize];
        n /= 58;
    }
    let s = core::str::from_utf8(&buf).expect("valid utf8 base58 hash");
    String::from_str(env, s)
}

// ---------------------------------------------------------------------------
// Issue #198 – activate_pet panics with PetNotFound for non-existent pet
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "PetNotFound")]
fn test_activate_pet_nonexistent_panics_with_pet_not_found() {
    let (_env, client, _admin) = setup_env();
    // Pet ID 9999 was never registered.
    client.activate_pet(&9999u64);
}

// ---------------------------------------------------------------------------
// Issue #198 – deactivate_pet panics with PetNotFound for non-existent pet
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "PetNotFound")]
fn test_deactivate_pet_nonexistent_panics_with_pet_not_found() {
    let (_env, client, _admin) = setup_env();
    client.deactivate_pet(&9999u64);
}

// ---------------------------------------------------------------------------
// Issue #199 – activate_pet panics with PetIsArchived for an archived pet
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "PetIsArchived")]
fn test_activate_archived_pet_panics_with_pet_is_archived() {
    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    // Archive the pet first.
    client.archive_pet(&pet_id);

    // Attempting to activate an archived pet must fail.
    client.activate_pet(&pet_id);
}

/// Unarchiving first then activating must succeed (lifecycle happy path).
#[test]
fn test_activate_pet_succeeds_after_unarchive() {
    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    client.archive_pet(&pet_id);
    client.unarchive_pet(&pet_id);
    // Must not panic — pet is no longer archived.
    client.activate_pet(&pet_id);
    assert!(client.is_pet_active(&pet_id));
}

// ---------------------------------------------------------------------------
// Issue #200 – archive_pet emits PetArchivedEvent
// ---------------------------------------------------------------------------

#[test]
fn test_archive_pet_emits_pet_archived_event() {
    use soroban_sdk::{testutils::Events, xdr::ContractEventBody, TryIntoVal, Val};

    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    let events_before = env.events().all().events().len();
    client.archive_pet(&pet_id);

    let all_snapshot = env.events().all();
    let all_events = all_snapshot.events();
    assert!(
        all_events.len() > events_before,
        "archive_pet must emit at least one event"
    );

    let target_topic = String::from_str(&env, "PetArchived");
    let mut found = false;

    for event in all_events.iter() {
        let ContractEventBody::V0(body) = &event.body;
        let mut topics: soroban_sdk::Vec<Val> = soroban_sdk::Vec::new(&env);
        for sv in body.topics.iter() {
            topics.push_back(sv.clone().try_into_val(&env).unwrap());
        }
        if topics.len() == 0 {
            continue;
        }
        let t0: Val = topics.get(0).unwrap();
        if let Ok(s) = String::try_from_val(&env, &t0) {
            if s == target_topic {
                let ev: crate::PetArchivedEvent =
                    body.data.clone().try_into_val(&env).unwrap();
                assert_eq!(ev.pet_id, pet_id, "PetArchivedEvent.pet_id must match");
                assert_eq!(ev.owner, owner, "PetArchivedEvent.owner must match");
                assert_eq!(
                    ev.version,
                    crate::EVENT_SCHEMA_VERSION,
                    "PetArchivedEvent.version must match EVENT_SCHEMA_VERSION"
                );
                found = true;
                break;
            }
        }
    }

    assert!(found, "PetArchivedEvent was not emitted by archive_pet");
}

// ---------------------------------------------------------------------------
// Issue #200 – unarchive_pet emits PetUnarchivedEvent
// ---------------------------------------------------------------------------

#[test]
fn test_unarchive_pet_emits_pet_unarchived_event() {
    use soroban_sdk::{testutils::Events, xdr::ContractEventBody, TryIntoVal, Val};

    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    // Archive first so we can unarchive.
    client.archive_pet(&pet_id);

    let events_before = env.events().all().events().len();
    client.unarchive_pet(&pet_id);

    let all_snapshot = env.events().all();
    let all_events = all_snapshot.events();
    assert!(
        all_events.len() > events_before,
        "unarchive_pet must emit at least one event"
    );

    let target_topic = String::from_str(&env, "PetUnarchived");
    let mut found = false;

    for event in all_events.iter() {
        let ContractEventBody::V0(body) = &event.body;
        let mut topics: soroban_sdk::Vec<Val> = soroban_sdk::Vec::new(&env);
        for sv in body.topics.iter() {
            topics.push_back(sv.clone().try_into_val(&env).unwrap());
        }
        if topics.len() == 0 {
            continue;
        }
        let t0: Val = topics.get(0).unwrap();
        if let Ok(s) = String::try_from_val(&env, &t0) {
            if s == target_topic {
                let ev: crate::PetUnarchivedEvent =
                    body.data.clone().try_into_val(&env).unwrap();
                assert_eq!(ev.pet_id, pet_id, "PetUnarchivedEvent.pet_id must match");
                assert_eq!(ev.owner, owner, "PetUnarchivedEvent.owner must match");
                assert_eq!(
                    ev.version,
                    crate::EVENT_SCHEMA_VERSION,
                    "PetUnarchivedEvent.version must match EVENT_SCHEMA_VERSION"
                );
                found = true;
                break;
            }
        }
    }

    assert!(found, "PetUnarchivedEvent was not emitted by unarchive_pet");
}

// ---------------------------------------------------------------------------
// Issue #201 – add_pet_photo rejects duplicate photo hashes
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "DuplicateEntry")]
fn test_add_duplicate_photo_hash_panics_with_duplicate_entry() {
    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    let hash = String::from_str(&env, "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");

    // First add must succeed.
    assert!(client.add_pet_photo(&pet_id, &hash));

    // Second add of the same hash must panic.
    client.add_pet_photo(&pet_id, &hash);
}

// ---------------------------------------------------------------------------
// Issue #201 – add_pet_photo rejects a 51st photo with StorageQuotaExceeded
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "StorageQuotaExceeded")]
fn test_add_photo_beyond_max_limit_panics_with_storage_quota_exceeded() {
    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    // Add exactly MAX_PET_PHOTOS (50) unique photos — all must succeed.
    for i in 0..50u32 {
        let hash = photo_hash_for_index(&env, i);
        assert!(
            client.add_pet_photo(&pet_id, &hash),
            "photo {} should be accepted",
            i
        );
    }

    // The 51st photo must panic with StorageQuotaExceeded.
    let overflow_hash = photo_hash_for_index(&env, 50u32);
    client.add_pet_photo(&pet_id, &overflow_hash);
}

/// Verify that exactly MAX_PET_PHOTOS photos are stored after filling up.
#[test]
fn test_add_pet_photo_count_at_max() {
    let (env, client, _admin) = setup_env();
    let owner = Address::generate(&env);
    let pet_id = register_pet(&client, &env, &owner);

    for i in 0..50u32 {
        let hash = photo_hash_for_index(&env, i);
        assert!(client.add_pet_photo(&pet_id, &hash));
    }

    assert_eq!(
        client.get_pet_photo_count(&pet_id),
        50u64,
        "photo count must be exactly 50 after filling the limit"
    );
}
