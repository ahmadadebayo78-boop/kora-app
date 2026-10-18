use crate::{
    AccessLevel, Gender, KoraContract, KoraContractClient, PrivacyLevel, Species,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String, Vec,
};

fn setup_env() -> (Env, KoraContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, KoraContract);
    let client = KoraContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    client.init_admin(&admin);
    (env, client, admin, owner)
}

fn register_test_pet(client: &KoraContractClient<'_>, env: &Env, owner: &Address) -> u64 {
    client.register_pet(
        owner,
        &String::from_str(env, "Buddy"),
        &String::from_str(env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(env, "Golden"),
        &String::from_str(env, "Golden"),
        &25,
        &None,
        &PrivacyLevel::Public,
    )
}

// ===========================================================================
// Issue #202: Return ContractError::PetNotFound and validate recipient in transfer_pet_ownership
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #24)")]
fn test_transfer_pet_ownership_panics_on_nonexistent_pet() {
    let (env, client, _admin, _owner) = setup_env();
    let recipient = Address::generate(&env);
    client.transfer_pet_ownership(&9999, &recipient, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #169)")]
fn test_transfer_pet_ownership_panics_on_self_transfer() {
    let (env, client, _admin, owner) = setup_env();
    let pet_id = register_test_pet(&client, &env, &owner);
    client.transfer_pet_ownership(&pet_id, &owner, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #170)")]
fn test_transfer_pet_ownership_panics_on_archived_pet() {
    let (env, client, _admin, owner) = setup_env();
    let pet_id = register_test_pet(&client, &env, &owner);
    let recipient = Address::generate(&env);
    client.archive_pet(&pet_id);
    client.transfer_pet_ownership(&pet_id, &recipient, &0);
}

#[test]
fn test_transfer_pet_ownership_succeeds_valid() {
    let (env, client, _admin, owner) = setup_env();
    let pet_id = register_test_pet(&client, &env, &owner);
    let recipient = Address::generate(&env);
    client.transfer_pet_ownership(&pet_id, &recipient, &0);
}

// ===========================================================================
// Issue #203: Validate future expiration timestamp and prevent self-grant in grant_access
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_grant_access_panics_on_expired_timestamp() {
    let (env, client, _admin, owner) = setup_env();
    env.ledger().with_mut(|l| l.timestamp = 1000);
    let pet_id = register_test_pet(&client, &env, &owner);
    let grantee = Address::generate(&env);
    client.grant_access(&pet_id, &grantee, &AccessLevel::Read, &Some(999), &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_grant_access_panics_on_current_timestamp() {
    let (env, client, _admin, owner) = setup_env();
    env.ledger().with_mut(|l| l.timestamp = 1000);
    let pet_id = register_test_pet(&client, &env, &owner);
    let grantee = Address::generate(&env);
    client.grant_access(&pet_id, &grantee, &AccessLevel::Read, &Some(1000), &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_grant_access_panics_on_self_grant() {
    let (env, client, _admin, owner) = setup_env();
    env.ledger().with_mut(|l| l.timestamp = 1000);
    let pet_id = register_test_pet(&client, &env, &owner);
    client.grant_access(&pet_id, &owner, &AccessLevel::Read, &Some(2000), &0);
}

#[test]
fn test_grant_access_succeeds_with_future_expiry() {
    let (env, client, _admin, owner) = setup_env();
    env.ledger().with_mut(|l| l.timestamp = 1000);
    let pet_id = register_test_pet(&client, &env, &owner);
    let grantee = Address::generate(&env);
    let ok = client.grant_access(&pet_id, &grantee, &AccessLevel::Read, &Some(2000), &0);
    assert!(ok);
}

// ===========================================================================
// Issue #204: Enforce string length validation and non-empty checks in update_owner_profile
// ===========================================================================

#[test]
#[should_panic(expected = "Error(Contract, #12)")]
fn test_update_owner_profile_panics_on_empty_name() {
    let (env, client, _admin, owner) = setup_env();
    client.register_pet_owner(
        &owner,
        &String::from_str(&env, "Alice"),
        &String::from_str(&env, "alice@example.com"),
        &String::from_str(&env, "123456"),
    );

    client.update_owner_profile(
        &owner,
        &String::from_str(&env, ""),
        &String::from_str(&env, "alice@example.com"),
        &String::from_str(&env, "123456"),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_update_owner_profile_panics_on_300_char_name() {
    let (env, client, _admin, owner) = setup_env();
    client.register_pet_owner(
        &owner,
        &String::from_str(&env, "Alice"),
        &String::from_str(&env, "alice@example.com"),
        &String::from_str(&env, "123456"),
    );

    let long_name = String::from_str(
        &env,
        "012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789",
    );
    assert_eq!(long_name.len(), 300);

    client.update_owner_profile(
        &owner,
        &long_name,
        &String::from_str(&env, "alice@example.com"),
        &String::from_str(&env, "123456"),
    );
}

#[test]
fn test_update_owner_profile_succeeds_valid() {
    let (env, client, _admin, owner) = setup_env();
    client.register_pet_owner(
        &owner,
        &String::from_str(&env, "Alice"),
        &String::from_str(&env, "alice@example.com"),
        &String::from_str(&env, "123456"),
    );

    let ok = client.update_owner_profile(
        &owner,
        &String::from_str(&env, "Alice Smith"),
        &String::from_str(&env, "asmith@example.com"),
        &String::from_str(&env, "987654"),
    );
    assert!(ok);
}

// ===========================================================================
// Issue #205: Maintain verified vet index and count to prevent unbounded scans in get_verified_vets
// ===========================================================================

#[test]
fn test_verified_vets_index_and_benchmark() {
    let (env, client, admin, _owner) = setup_env();

    // Register 50 unverified vets
    for i in 0..50 {
        let vet = Address::generate(&env);
        let lic = String::from_str(&env, "LIC-UNVER");
        client.register_vet(
            &vet,
            &String::from_str(&env, "Dr. Unverified"),
            &lic,
            &String::from_str(&env, "General"),
        );
    }

    // Register and verify 5 vets
    let mut verified_vets = Vec::new(&env);
    for _ in 0..5 {
        let vet = Address::generate(&env);
        let lic = String::from_str(&env, "LIC-VER");
        client.register_vet(
            &vet,
            &String::from_str(&env, "Dr. Verified"),
            &lic,
            &String::from_str(&env, "General"),
        );
        client.verify_vet(&admin, &vet);
        verified_vets.push_back(vet);
    }

    // Querying verified vets returns only the 5 verified vets
    let list = client.get_verified_vets(&0, &10);
    assert_eq!(list.len(), 5);
    for i in 0..5 {
        assert_eq!(list.get(i as u32).unwrap().address, verified_vets.get(i as u32).unwrap());
    }

    // Pagination bounds check: offset 2, limit 2
    let page = client.get_verified_vets(&2, &2);
    assert_eq!(page.len(), 2);
    assert_eq!(page.get(0).unwrap().address, verified_vets.get(2).unwrap());
    assert_eq!(page.get(1).unwrap().address, verified_vets.get(3).unwrap());

    // Revoking vet #2 updates index and count
    let revoked_addr = verified_vets.get(2).unwrap();
    client.revoke_vet_license(&admin, &revoked_addr);
    let after_revoke = client.get_verified_vets(&0, &10);
    assert_eq!(after_revoke.len(), 4);
    for i in 0..after_revoke.len() {
        assert_ne!(after_revoke.get(i).unwrap().address, revoked_addr);
    }
}
