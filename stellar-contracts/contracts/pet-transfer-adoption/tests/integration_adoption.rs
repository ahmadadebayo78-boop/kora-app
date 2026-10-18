use pet_transfer_adoption::{
    ContractError, PetOwnershipContract, PetOwnershipContractClient, TransferType,
};
use soroban_sdk::{testutils::Address as _, Address, Env, Error};

#[test]
fn adoption_flow_transfers_pet_ownership() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let adopter = Address::generate(&env);
    let pet_id = 7;

    client.create_pet(&pet_id, &owner);
    client.set_adoption_config(&0, &owner);
    client.sign_adoption(&pet_id, &adopter, &None);
    client.approve_adoption(&pet_id, &adopter);
    client.complete_adoption(&pet_id);

    assert_eq!(client.get_current_owner(&pet_id), adopter);
    assert_eq!(client.get_adoption_record(&pet_id).unwrap().pet_id, pet_id);
}

/// Issue #111 – complete_adoption must record a CustodyEntry so the adoption
/// is reflected in the chain-of-custody provenance history.
#[test]
fn adoption_flow_records_custody_chain_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let adopter = Address::generate(&env);
    let pet_id = 8;

    client.create_pet(&pet_id, &owner);
    client.set_adoption_config(&0, &owner);
    client.sign_adoption(&pet_id, &adopter, &None);
    client.approve_adoption(&pet_id, &adopter);
    client.complete_adoption(&pet_id);

    // The custody chain must contain exactly one entry reflecting the adoption.
    let chain = client.get_custody_chain(&pet_id);
    assert_eq!(
        chain.len(),
        1,
        "complete_adoption must append exactly one custody entry"
    );

    let entry = chain.get(0).unwrap();
    assert_eq!(
        entry.from, owner,
        "custody entry must record the previous owner"
    );
    assert_eq!(entry.to, adopter, "custody entry must record the new owner");
    assert_eq!(
        entry.transfer_type,
        TransferType::Adoption,
        "adoption transfers must be logged with TransferType::Adoption"
    );
}

/// Issue #111 – finalize_transfer must record a CustodyEntry so direct
/// transfers are reflected in the chain-of-custody provenance history.
#[test]
fn finalize_transfer_records_custody_chain_entry() {
    use pet_transfer_adoption::DISPUTE_WINDOW_SECONDS;
    use soroban_sdk::testutils::Ledger;

    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let pet_id = 9;

    client.create_pet(&pet_id, &owner);
    client.initiate_transfer(&pet_id, &new_owner);
    client.accept_transfer(&pet_id);

    // Advance past the dispute window so finalize_transfer can proceed.
    env.ledger()
        .with_mut(|l| l.timestamp += DISPUTE_WINDOW_SECONDS + 1);
    client.finalize_transfer(&pet_id, &owner);

    // The custody chain must contain exactly one entry reflecting the transfer.
    let chain = client.get_custody_chain(&pet_id);
    assert_eq!(
        chain.len(),
        1,
        "finalize_transfer must append exactly one custody entry"
    );

    let entry = chain.get(0).unwrap();
    assert_eq!(
        entry.from, owner,
        "custody entry must record the previous owner"
    );
    assert_eq!(
        entry.to, new_owner,
        "custody entry must record the new owner"
    );
    assert_eq!(
        entry.transfer_type,
        TransferType::Direct,
        "direct transfers must be logged with TransferType::Direct"
    );
}

// ======================================================
// Issues #190, #191, #192, #193 tests
// ======================================================

/// Issue #190 – finalize_transfer must enforce party authorization
#[test]
fn finalize_transfer_party_authorization() {
    use pet_transfer_adoption::DISPUTE_WINDOW_SECONDS;
    use soroban_sdk::testutils::Ledger;

    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let stranger = Address::generate(&env);
    let pet_id = 10;

    client.create_pet(&pet_id, &owner);
    client.initiate_transfer(&pet_id, &new_owner);
    client.accept_transfer(&pet_id);

    // Advance past dispute window
    env.ledger()
        .with_mut(|l| l.timestamp += DISPUTE_WINDOW_SECONDS + 1);

    // Third-party caller must be rejected with Unauthorized
    let result = client.try_finalize_transfer(&pet_id, &stranger);
    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::Unauthorized as u32,
        )))
    );

    // The recipient ('to') can finalize the transfer
    client.finalize_transfer(&pet_id, &new_owner);
    assert_eq!(client.get_current_owner(&pet_id), new_owner);
}

#[test]
fn finalize_transfer_sender_authorization() {
    use pet_transfer_adoption::DISPUTE_WINDOW_SECONDS;
    use soroban_sdk::testutils::Ledger;

    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let pet_id = 11;

    client.create_pet(&pet_id, &owner);
    client.initiate_transfer(&pet_id, &new_owner);
    client.accept_transfer(&pet_id);

    env.ledger()
        .with_mut(|l| l.timestamp += DISPUTE_WINDOW_SECONDS + 1);

    // The sender ('from') can finalize the transfer
    client.finalize_transfer(&pet_id, &owner);
    assert_eq!(client.get_current_owner(&pet_id), new_owner);
}

/// Issue #191 – initiate_transfer_with_timeout upper bound enforcement
#[test]
fn initiate_transfer_with_timeout_bounds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let pet_id = 12;

    client.create_pet(&pet_id, &owner);

    // transfer_timeout_days = 91 must be rejected with InvalidTimeoutDays
    let result = client.try_initiate_transfer_with_timeout(&pet_id, &new_owner, &91u32);
    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::InvalidTimeoutDays as u32,
        )))
    );

    // transfer_timeout_days = 30 must succeed
    client.initiate_transfer_with_timeout(&pet_id, &new_owner, &30u32);
    assert!(client.has_pending_transfer(&pet_id));
}

/// Issue #192 – sign_adoption rejects owner adopting pet to themselves
#[test]
fn sign_adoption_rejects_self_adoption() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let adopter = Address::generate(&env);
    let pet_id = 13;

    client.create_pet(&pet_id, &owner);

    // to == pet.current_owner must fail with InvalidRecipient
    let result = client.try_sign_adoption(&pet_id, &owner, &None);
    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::InvalidRecipient as u32,
        )))
    );

    // to != pet.current_owner must succeed
    client.sign_adoption(&pet_id, &adopter, &None);
    assert!(client.get_pending_adoption(&pet_id).is_some());
}

/// Issue #193 – create_pet rejects pet_id == 0
#[test]
fn create_pet_validates_pet_id_positive() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PetOwnershipContract);
    let client = PetOwnershipContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);

    // pet_id == 0 must fail with InvalidInput
    let result = client.try_create_pet(&0u64, &owner);
    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::InvalidInput as u32,
        )))
    );

    // pet_id >= 1 succeeds
    client.create_pet(&1u64, &owner);
    assert_eq!(client.get_current_owner(&1u64), owner);
}
