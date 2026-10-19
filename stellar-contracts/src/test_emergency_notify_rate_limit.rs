use crate::*;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::Env;

fn setup_pet_with_contact(
    env: &Env,
    client: &KoraContractClient,
    owner: &Address,
) -> u64 {
    let pet_id = client.register_pet(
        owner,
        &String::from_str(env, "Buddy"),
        &String::from_str(env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(env, "Golden Retriever"),
        &String::from_str(env, "Golden"),
        &25u32,
        &None,
        &PrivacyLevel::Public,
    );

    let mut contacts = Vec::new(env);
    contacts.push_back(EmergencyContact {
        name: String::from_str(env, "Jane Doe"),
        phone: String::from_str(env, "555-0100"),
        email: String::from_str(env, "jane@example.com"),
        relationship: String::from_str(env, "Vet"),
        is_primary: true,
        priority: 1,
    });

    client.set_emergency_contacts(
        &pet_id,
        &contacts,
        &Vec::new(env),
        &String::from_str(env, ""),
    );

    pet_id
}

#[test]
fn first_three_calls_succeed() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = setup_pet_with_contact(&env, &client, &owner);

    for _ in 0..3 {
        let notified = client.notify_emergency_contacts(&owner, &pet_id);
        assert_eq!(notified, 1);
    }
}

#[test]
fn fourth_call_within_the_hour_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = setup_pet_with_contact(&env, &client, &owner);

    for _ in 0..3 {
        client.notify_emergency_contacts(&owner, &pet_id);
    }

    let result = client.try_notify_emergency_contacts(&owner, &pet_id);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::RateLimitExceeded.into()
    );
}

#[test]
fn limit_resets_after_the_window_elapses() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = setup_pet_with_contact(&env, &client, &owner);

    for _ in 0..3 {
        client.notify_emergency_contacts(&owner, &pet_id);
    }
    assert!(client.try_notify_emergency_contacts(&owner, &pet_id).is_err());

    let current = env.ledger().timestamp();
    env.ledger().set_timestamp(current + 3_601);

    let notified = client.notify_emergency_contacts(&owner, &pet_id);
    assert_eq!(notified, 1);
}

#[test]
fn rate_limit_is_scoped_per_caller_and_pet() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = setup_pet_with_contact(&env, &client, &owner);

    let responder = Address::generate(&env);
    client.add_emergency_responder(&pet_id, &responder);

    for _ in 0..3 {
        client.notify_emergency_contacts(&owner, &pet_id);
    }
    assert!(client.try_notify_emergency_contacts(&owner, &pet_id).is_err());

    // A different authorized caller for the same pet has its own limit.
    let notified = client.notify_emergency_contacts(&responder, &pet_id);
    assert_eq!(notified, 1);
}

#[test]
fn per_pet_rate_limit_blocks_across_different_callers() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = setup_pet_with_contact(&env, &client, &owner);

    // Add 4 more responders so each can make calls under their own per-caller limit
    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let r3 = Address::generate(&env);
    let r4 = Address::generate(&env);
    client.add_emergency_responder(&pet_id, &r1);
    client.add_emergency_responder(&pet_id, &r2);
    client.add_emergency_responder(&pet_id, &r3);
    client.add_emergency_responder(&pet_id, &r4);

    // Each distinct caller makes 1 call — 5 total, exhausting the per-pet limit
    client.notify_emergency_contacts(&owner, &pet_id);
    client.notify_emergency_contacts(&r1, &pet_id);
    client.notify_emergency_contacts(&r2, &pet_id);
    client.notify_emergency_contacts(&r3, &pet_id);
    client.notify_emergency_contacts(&r4, &pet_id);

    // A fresh caller is still under their own per-caller limit, but the pet
    // has exhausted its 5-per-hour budget — must be rejected.
    let r5 = Address::generate(&env);
    client.add_emergency_responder(&pet_id, &r5);
    let result = client.try_notify_emergency_contacts(&r5, &pet_id);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        ContractError::RateLimitExceeded.into()
    );
}

#[test]
fn per_pet_rate_limit_resets_after_window() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = setup_pet_with_contact(&env, &client, &owner);

    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let r3 = Address::generate(&env);
    let r4 = Address::generate(&env);
    client.add_emergency_responder(&pet_id, &r1);
    client.add_emergency_responder(&pet_id, &r2);
    client.add_emergency_responder(&pet_id, &r3);
    client.add_emergency_responder(&pet_id, &r4);

    // Exhaust the per-pet budget
    client.notify_emergency_contacts(&owner, &pet_id);
    client.notify_emergency_contacts(&r1, &pet_id);
    client.notify_emergency_contacts(&r2, &pet_id);
    client.notify_emergency_contacts(&r3, &pet_id);
    client.notify_emergency_contacts(&r4, &pet_id);

    assert!(client.try_notify_emergency_contacts(&owner, &pet_id).is_err());

    // Advance time past the 1-hour window
    let current = env.ledger().timestamp();
    env.ledger().set_timestamp(current + 3_601);

    // The per-pet counter resets, so this should succeed again
    let notified = client.notify_emergency_contacts(&owner, &pet_id);
    assert_eq!(notified, 1);
}
