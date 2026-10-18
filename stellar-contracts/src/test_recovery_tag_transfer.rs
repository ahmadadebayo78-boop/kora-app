//! Regression tests for Issue #214: a pet's recovery tag must report the
//! current legal owner after an accepted ownership transfer.
//!
//! `PetTag.owner` is snapshotted when the tag is linked, so before this fix a
//! lost pet's `get_tag` kept reporting the former owner once ownership moved.

use crate::{DataKey, Gender, KoraContract, KoraContractClient, Pet, PrivacyLevel, Species, TransferType};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

/// `register_pet` persists the pet via `store_pet` (persistent storage) while
/// the transfer/tag entrypoints read `DataKey::Pet` from instance storage.
/// Copy the pet entry into instance storage so both halves observe the same
/// pet — the same seed-what-the-target-reads convention used by
/// `test_attachment_limit.rs`.
fn mirror_pet_into_instance_storage(env: &Env, contract_id: &Address, pet_id: u64) {
    env.as_contract(contract_id, || {
        if let Some(pet) = env
            .storage()
            .persistent()
            .get::<DataKey, Pet>(&DataKey::Pet(pet_id))
        {
            env.storage().instance().set(&DataKey::Pet(pet_id), &pet);
        }
    });
}

fn setup() -> (Env, KoraContractClient<'static>, Address, u64) {
    let env = Env::default();
    env.mock_all_auths();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, KoraContract);
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Buddy"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Golden Retriever"),
        &String::from_str(&env, "Golden"),
        &30u32,
        &None,
        &PrivacyLevel::Public,
    );
    mirror_pet_into_instance_storage(&env, &contract_id, pet_id);

    (env, client, owner, pet_id)
}

/// Transfer acceptance must update the linked tag's owner so `get_tag`
/// reports the current legal owner, not the former one.
#[test]
fn test_tag_owner_updates_on_transfer_acceptance() {
    let (env, client, owner, pet_id) = setup();

    let tag_id = client.link_tag_to_pet(&pet_id);
    assert_eq!(client.get_tag(&tag_id).unwrap().owner, owner);

    let new_owner = Address::generate(&env);
    client.transfer_pet_ownership(&pet_id, &new_owner, &0);
    client.accept_pet_transfer(&pet_id, &TransferType::Direct, &None);

    let tag = client.get_tag(&tag_id).unwrap();
    assert_eq!(
        tag.owner, new_owner,
        "get_tag must report the current legal owner after acceptance",
    );
}

/// A pending (not yet accepted) transfer must NOT change the tag owner — the
/// former owner stays legally responsible until the recipient accepts.
#[test]
fn test_tag_owner_unchanged_while_transfer_pending() {
    let (env, client, owner, pet_id) = setup();

    let tag_id = client.link_tag_to_pet(&pet_id);
    let new_owner = Address::generate(&env);
    client.transfer_pet_ownership(&pet_id, &new_owner, &0);

    assert_eq!(client.get_tag(&tag_id).unwrap().owner, owner);
}

/// Accepting a transfer for a pet without a linked tag must not panic.
#[test]
fn test_transfer_acceptance_without_linked_tag_succeeds() {
    let (env, client, _owner, pet_id) = setup();

    let new_owner = Address::generate(&env);
    client.transfer_pet_ownership(&pet_id, &new_owner, &0);
    client.accept_pet_transfer(&pet_id, &TransferType::Direct, &None);

    assert!(client.get_tag_by_pet(&pet_id).is_none());
}
