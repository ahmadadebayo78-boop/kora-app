//! Tests for Issue #215: `remove_attachment` lets the authoring vet or the
//! pet owner delete an attachment from a medical record and releases the
//! storage quota the attachment consumed.

use crate::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

/// A valid CIDv0 IPFS hash (46 chars, base58, `Qm` prefix).
const IPFS_HASH_A: &str = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
/// A second valid CIDv0 IPFS hash, distinct from `IPFS_HASH_A`.
const IPFS_HASH_B: &str = "QmT5NvUtoM5nWFfrQdVrFtvGfKFmG7AHE8P34isapyhCxX";

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

fn mirror_record_into_instance_storage(env: &Env, contract_id: &Address, record_id: u64) {
    env.as_contract(contract_id, || {
        if let Some(record) = env
            .storage()
            .persistent()
            .get::<MedicalKey, MedicalRecord>(&MedicalKey::MedicalRecord(record_id))
        {
            env.storage()
                .instance()
                .set(&MedicalKey::MedicalRecord(record_id), &record);
            env.storage()
                .persistent()
                .remove(&MedicalKey::MedicalRecord(record_id));
        }
    });
}

fn setup() -> (Env, KoraContractClient<'static>, Address, Address, u64, u64) {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init_admin(&admin);

    let owner = Address::generate(&env);
    let vet = Address::generate(&env);
    client.register_vet(
        &vet,
        &String::from_str(&env, "Dr. Smith"),
        &String::from_str(&env, "LIC-ATT-001"),
        &String::from_str(&env, "General Practice"),
    );
    client.verify_vet(&admin, &vet);

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
    // add_medical_record reads the pet from INSTANCE storage, but register_pet
    // persisted it to PERSISTENT storage. Mirror into instance first.
    mirror_pet_into_instance_storage(&env, &contract_id, pet_id);

    let record_id = client.add_medical_record(
        &pet_id,
        &vet,
        &String::from_str(&env, "Annual checkup"),
        &String::from_str(&env, "Healthy"),
        &Vec::new(&env),
        &String::from_str(&env, "All vitals normal"),
    );
    // add_attachment/remove_attachment read the record from INSTANCE storage
    // only, while add_medical_record wrote it to PERSISTENT storage. Mirror
    // the record into instance and drop the persistent copy so
    // load_medical_record (persistent-first) observes instance updates.
    mirror_record_into_instance_storage(&env, &contract_id, record_id);

    (env, client, vet, owner, pet_id, record_id)
}

fn metadata(env: &Env, filename: &str) -> AttachmentMetadata {
    AttachmentMetadata {
        filename: String::from_str(env, filename),
        file_type: String::from_str(env, "image/jpeg"),
        size: 1024,
        uploaded_date: env.ledger().timestamp(),
    }
}

fn add_two_attachments(env: &Env, client: &KoraContractClient, record_id: &u64) {
    assert!(client.add_attachment(
        record_id,
        &String::from_str(env, IPFS_HASH_A),
        &metadata(env, "xray.jpg"),
        &BytesN::from_array(env, &[1u8; 32]),
    ));
    assert!(client.add_attachment(
        record_id,
        &String::from_str(env, IPFS_HASH_B),
        &metadata(env, "bloodwork.jpg"),
        &BytesN::from_array(env, &[2u8; 32]),
    ));
    assert_eq!(client.get_attachment_count(record_id), 2);
}

/// The authoring vet can remove an attachment; the other attachment remains.
#[test]
fn test_remove_attachment_by_vet_succeeds() {
    let (env, client, vet, _owner, _pet_id, record_id) = setup();
    add_two_attachments(&env, &client, &record_id);

    assert!(client.remove_attachment(
        &record_id,
        &String::from_str(&env, IPFS_HASH_A),
        &vet,
    ));

    assert_eq!(client.get_attachment_count(&record_id), 1);
    let attachments = client.get_attachments(&record_id);
    assert_eq!(attachments.len(), 1);
    assert_eq!(
        attachments.get(0).unwrap().ipfs_hash,
        String::from_str(&env, IPFS_HASH_B)
    );
}

/// The pet's current owner can also remove an attachment.
#[test]
fn test_remove_attachment_by_owner_succeeds() {
    let (env, client, _vet, owner, _pet_id, record_id) = setup();
    add_two_attachments(&env, &client, &record_id);

    assert!(client.remove_attachment(
        &record_id,
        &String::from_str(&env, IPFS_HASH_B),
        &owner,
    ));

    assert_eq!(client.get_attachment_count(&record_id), 1);
    let attachments = client.get_attachments(&record_id);
    assert_eq!(
        attachments.get(0).unwrap().ipfs_hash,
        String::from_str(&env, IPFS_HASH_A)
    );
}

/// A caller that is neither the authoring vet nor the pet owner is rejected
/// with `ContractError::Unauthorized`, and the attachment list is untouched.
#[test]
fn test_unauthorized_caller_cannot_remove_attachment() {
    let (env, client, _vet, _owner, _pet_id, record_id) = setup();
    add_two_attachments(&env, &client, &record_id);

    let stranger = Address::generate(&env);
    let result = client.try_remove_attachment(
        &record_id,
        &String::from_str(&env, IPFS_HASH_A),
        &stranger,
    );
    let expected = soroban_sdk::Error::from_contract_error(ContractError::Unauthorized as u32);
    assert_eq!(result, Err(Ok(expected)));

    assert_eq!(client.get_attachment_count(&record_id), 2);
}

/// Removing an attachment that does not match any stored IPFS hash fails with
/// `ContractError::InvalidInput`.
#[test]
fn test_remove_nonexistent_attachment_rejected() {
    let (env, client, vet, _owner, _pet_id, record_id) = setup();
    add_two_attachments(&env, &client, &record_id);

    let result = client.try_remove_attachment(
        &record_id,
        &String::from_str(&env, "QmPK1s3pNYLi9ERiq3BDxKa4XosgWwFRQUydHUtz4YgpqB"),
        &vet,
    );
    let expected = soroban_sdk::Error::from_contract_error(ContractError::InvalidInput as u32);
    assert_eq!(result, Err(Ok(expected)));

    assert_eq!(client.get_attachment_count(&record_id), 2);
}

/// Removing an attachment must release the storage quota unit that
/// `add_attachment` consumed.
#[test]
fn test_remove_attachment_releases_storage_quota() {
    let (env, client, vet, _owner, pet_id, record_id) = setup();

    let baseline = client.get_storage_usage(&pet_id).current_count;

    client.add_attachment(
        &record_id,
        &String::from_str(&env, IPFS_HASH_A),
        &metadata(&env, "xray.jpg"),
        &BytesN::from_array(&env, &[1u8; 32]),
    );
    assert_eq!(client.get_storage_usage(&pet_id).current_count, baseline + 1);

    client.remove_attachment(&record_id, &String::from_str(&env, IPFS_HASH_A), &vet);
    assert_eq!(client.get_storage_usage(&pet_id).current_count, baseline);
}
