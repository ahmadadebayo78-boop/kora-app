/// Tests for issues #206, #207, #208, and #209.
///
/// #206 – add_vaccination panics with ContractError::VetNotVerified (not a
///         plain string panic) when an unverified veterinarian attempts to
///         record a vaccination.
///
/// #207 – add_vaccination validates:
///          * administered_at <= now  (InvalidInput on future date)
///          * next_due_date >= administered_at  (InvalidInput on bad order)
///          * vaccine_name and batch_number lengths <= MAX_STR_SHORT=100
///            (InputStringTooLong)
///
/// #208 – add_lab_result panics with ContractError::TooManyItems when the
///         biomarkers map contains more than MAX_BIOMARKERS_PER_RESULT=15
///         entries.
///
/// #209 – set_diet_plan panics with ContractError::TooManyItems when
///         restrictions or allergies exceed 10 items, and with
///         ContractError::InputStringTooLong when any individual item exceeds
///         MAX_STR_SHORT=100.
extern crate std;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, Map, String, Vec,
};
use std::{format, vec};

// ---------------------------------------------------------------------------
// Shared setup helpers
// ---------------------------------------------------------------------------

fn setup_env() -> (
    Env,
    crate::KoraContractClient<'static>,
    Address, // admin
    Address, // owner
    Address, // verified vet
    u64,     // pet_id
) {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let contract_id = env.register(crate::KoraContract, ());
    let client = crate::KoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let vet = Address::generate(&env);

    client.init_admin(&admin);
    client.register_vet(
        &vet,
        &String::from_str(&env, "Dr. Tester"),
        &String::from_str(&env, "LIC-206-TEST"),
        &String::from_str(&env, "General Practice"),
    );
    client.verify_vet(&admin, &vet);

    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Buddy"),
        &String::from_str(&env, "2020-01-01"),
        &crate::Gender::Male,
        &crate::Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Black"),
        &30u32,
        &None,
        &crate::PrivacyLevel::Public,
    );

    (env, client, admin, owner, vet, pet_id)
}

/// Helper to build a String of `n` repetitions of a single ASCII byte.
fn repeat_char(env: &Env, byte: u8, n: usize) -> String {
    let mut buf = vec![0u8; n];
    for b in buf.iter_mut() {
        *b = byte;
    }
    String::from_bytes(env, &buf)
}

// ---------------------------------------------------------------------------
// Issue #206 – structured error on unverified vet
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_206_unverified_vet_panics() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    // Generate a vet address that is NOT verified.
    let unverified_vet = Address::generate(&env);

    let now = env.ledger().timestamp();
    client.add_vaccination(
        &pet_id,
        &unverified_vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
}

#[test]
fn test_206_verified_vet_succeeds() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    let now = env.ledger().timestamp();
    let id = client.add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
    assert!(id > 0, "vaccination id must be positive");
}

// ---------------------------------------------------------------------------
// Issue #207 – administered_at must not be in the future
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_207_future_administered_at_rejected() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    // Set ledger timestamp to 1000 so "now" is non-zero.
    env.ledger().set_timestamp(1_000);
    let now = env.ledger().timestamp();

    // administered_at is in the future (now + 1 day).
    let future = now + 86_400;
    client.add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &future, // administered_at > now → InvalidInput
        &(future + 86_400),
        &(future + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
}

// ---------------------------------------------------------------------------
// Issue #207 – next_due_date must not be before administered_at
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_207_next_due_before_administered_rejected() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    env.ledger().set_timestamp(10_000);
    let now = env.ledger().timestamp();
    // next_due_date is earlier than administered_at.
    client.add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now - 1), // next_due_date < administered_at → InvalidInput
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
}

// ---------------------------------------------------------------------------
// Issue #207 – vaccine_name must not exceed MAX_STR_SHORT (100 bytes)
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_207_vaccine_name_over_limit_rejected() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    let now = env.ledger().timestamp();
    client.add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &repeat_char(&env, b'V', 101), // 101 bytes → InputStringTooLong
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
}

#[test]
fn test_207_vaccine_name_at_limit_accepted() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    let now = env.ledger().timestamp();
    let id = client.add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &repeat_char(&env, b'V', 100), // exactly 100 bytes → ok
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
    assert!(id > 0);
}

// ---------------------------------------------------------------------------
// Issue #207 – batch_number must not exceed MAX_STR_SHORT (100 bytes)
// ---------------------------------------------------------------------------

#[test]
#[should_panic]
fn test_207_batch_number_over_limit_rejected() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    let now = env.ledger().timestamp();
    client.add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &repeat_char(&env, b'B', 101), // 101 bytes → InputStringTooLong
    );
}

// ---------------------------------------------------------------------------
// Issue #208 – biomarkers map size capped at 15
// ---------------------------------------------------------------------------

/// Helper that calls add_lab_result with an explicit biomarkers map.
fn add_lab_result_with_biomarkers(
    env: &Env,
    client: &crate::KoraContractClient,
    pet_id: u64,
    vet: &Address,
    biomarkers: &Map<String, i128>,
) -> u64 {
    client.add_lab_result(
        &pet_id,
        vet,
        &String::from_str(env, "Blood Panel"),
        &String::from_str(env, "see attached"),
        &String::from_str(env, "0-100"),
        &0u32,
        &100u32,
        &None,
        &None,
        biomarkers,
    )
}

#[test]
#[should_panic]
fn test_208_too_many_biomarkers_rejected() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    // Build a map with 20 biomarkers (> MAX_BIOMARKERS_PER_RESULT=15).
    let mut bm: Map<String, i128> = Map::new(&env);
    for i in 0u32..20u32 {
        let key = String::from_str(&env, &format!("bm_{}", i));
        bm.set(key, i as i128);
    }

    add_lab_result_with_biomarkers(&env, &client, pet_id, &vet, &bm);
}

#[test]
fn test_208_exactly_15_biomarkers_accepted() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    // Build a map with exactly 15 biomarkers (== MAX_BIOMARKERS_PER_RESULT).
    let mut bm: Map<String, i128> = Map::new(&env);
    for i in 0u32..15u32 {
        let key = String::from_str(&env, &format!("bm_{}", i));
        bm.set(key, i as i128);
    }

    let id = add_lab_result_with_biomarkers(&env, &client, pet_id, &vet, &bm);
    assert!(id > 0);
}

#[test]
fn test_208_empty_biomarkers_accepted() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();

    let bm: Map<String, i128> = Map::new(&env);
    let id = add_lab_result_with_biomarkers(&env, &client, pet_id, &vet, &bm);
    assert!(id > 0);
}

// ---------------------------------------------------------------------------
// Issue #209 – restrictions and allergies capped at 10 items
// ---------------------------------------------------------------------------

fn call_set_diet_plan(
    env: &Env,
    client: &crate::KoraContractClient,
    pet_id: u64,
    restrictions: &Vec<String>,
    allergies: &Vec<String>,
) -> bool {
    client.set_diet_plan(
        &pet_id,
        &String::from_str(env, "Dry Food"),
        &String::from_str(env, "250g"),
        &String::from_str(env, "Twice daily"),
        &300u32,
        &600u32,
        restrictions,
        allergies,
    )
}

#[test]
#[should_panic]
fn test_209_too_many_restrictions_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    // Build a restrictions vec with 20 items (> 10).
    let mut restrictions = Vec::new(&env);
    for i in 0u32..20u32 {
        restrictions.push_back(String::from_str(&env, &format!("restriction_{}", i)));
    }
    let allergies = Vec::new(&env);

    call_set_diet_plan(&env, &client, pet_id, &restrictions, &allergies);
}

#[test]
#[should_panic]
fn test_209_too_many_allergies_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    let restrictions = Vec::new(&env);
    // Build an allergies vec with 11 items (> 10).
    let mut allergies = Vec::new(&env);
    for i in 0u32..11u32 {
        allergies.push_back(String::from_str(&env, &format!("allergy_{}", i)));
    }

    call_set_diet_plan(&env, &client, pet_id, &restrictions, &allergies);
}

#[test]
fn test_209_exactly_10_restrictions_accepted() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    let mut restrictions = Vec::new(&env);
    for i in 0u32..10u32 {
        restrictions.push_back(String::from_str(&env, &format!("restriction_{}", i)));
    }
    let allergies = Vec::new(&env);

    let ok = call_set_diet_plan(&env, &client, pet_id, &restrictions, &allergies);
    assert!(ok);
}

#[test]
#[should_panic]
fn test_209_restriction_item_over_limit_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    let mut restrictions = Vec::new(&env);
    // One item with 101 bytes → InputStringTooLong.
    restrictions.push_back(repeat_char(&env, b'R', 101));
    let allergies = Vec::new(&env);

    call_set_diet_plan(&env, &client, pet_id, &restrictions, &allergies);
}

#[test]
#[should_panic]
fn test_209_allergy_item_over_limit_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    let restrictions = Vec::new(&env);
    let mut allergies = Vec::new(&env);
    // One item with 101 bytes → InputStringTooLong.
    allergies.push_back(repeat_char(&env, b'A', 101));

    call_set_diet_plan(&env, &client, pet_id, &restrictions, &allergies);
}

#[test]
fn test_209_restriction_item_at_limit_accepted() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    let mut restrictions = Vec::new(&env);
    // Exactly 100 bytes → ok.
    restrictions.push_back(repeat_char(&env, b'R', 100));
    let allergies = Vec::new(&env);

    let ok = call_set_diet_plan(&env, &client, pet_id, &restrictions, &allergies);
    assert!(ok);
}

#[test]
#[should_panic]
fn test_209_food_type_over_limit_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();
    client.set_diet_plan(
        &pet_id,
        &repeat_char(&env, b'F', 101),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );
}

#[test]
#[should_panic]
fn test_209_portion_size_over_limit_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();
    client.set_diet_plan(
        &pet_id,
        &String::from_str(&env, "Dry Food"),
        &repeat_char(&env, b'P', 101),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );
}

#[test]
#[should_panic]
fn test_209_frequency_over_limit_rejected() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();
    client.set_diet_plan(
        &pet_id,
        &String::from_str(&env, "Dry Food"),
        &String::from_str(&env, "250g"),
        &repeat_char(&env, b'Q', 101),
        &300u32,
        &600u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );
}

#[test]
fn test_206_unverified_vet_decoded_error() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();
    let unverified_vet = Address::generate(&env);
    let now = env.ledger().timestamp();
    let result = client.try_add_vaccination(
        &pet_id,
        &unverified_vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        crate::ContractError::VetNotVerified.into()
    );
}

#[test]
fn test_207_future_administered_at_decoded_error() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();
    env.ledger().set_timestamp(1_000);
    let now = env.ledger().timestamp();
    let future = now + 86_400;
    let result = client.try_add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &future,
        &(future + 86_400),
        &(future + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        crate::ContractError::InvalidInput.into()
    );
}

#[test]
fn test_207_next_due_before_administered_decoded_error() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();
    env.ledger().set_timestamp(10_000);
    let now = env.ledger().timestamp();
    let result = client.try_add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now - 1),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        crate::ContractError::InvalidInput.into()
    );
}

#[test]
fn test_207_overlong_strings_decoded_error() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();
    let now = env.ledger().timestamp();
    let result_name = client.try_add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &repeat_char(&env, b'V', 101),
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &String::from_str(&env, "BATCH-001"),
    );
    assert!(result_name.is_err());
    assert_eq!(
        result_name.unwrap_err().unwrap(),
        crate::ContractError::InputStringTooLong.into()
    );

    let result_batch = client.try_add_vaccination(
        &pet_id,
        &vet,
        &crate::VaccineType::Rabies,
        &String::from_str(&env, "RabiesVax"),
        &now,
        &(now + 86_400),
        &(now + 86_400),
        &repeat_char(&env, b'B', 101),
    );
    assert!(result_batch.is_err());
    assert_eq!(
        result_batch.unwrap_err().unwrap(),
        crate::ContractError::InputStringTooLong.into()
    );
}

#[test]
fn test_208_too_many_biomarkers_decoded_error() {
    let (env, client, _admin, _owner, vet, pet_id) = setup_env();
    let mut bm: Map<String, i128> = Map::new(&env);
    for i in 0u32..20u32 {
        let key = String::from_str(&env, &format!("bm_{}", i));
        bm.set(key, i as i128);
    }
    let result = client.try_add_lab_result(
        &pet_id,
        &vet,
        &String::from_str(&env, "Blood Panel"),
        &String::from_str(&env, "see attached"),
        &String::from_str(&env, "0-100"),
        &0u32,
        &100u32,
        &None,
        &None,
        &bm,
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().unwrap(),
        crate::ContractError::TooManyItems.into()
    );
}

#[test]
fn test_209_decoded_errors() {
    let (env, client, _admin, _owner, _vet, pet_id) = setup_env();

    // 1. > 10 restrictions -> TooManyItems
    let mut restrictions = Vec::new(&env);
    for i in 0u32..20u32 {
        restrictions.push_back(String::from_str(&env, &format!("restriction_{}", i)));
    }
    let res = client.try_set_diet_plan(
        &pet_id,
        &String::from_str(&env, "Dry Food"),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &restrictions,
        &Vec::new(&env),
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().unwrap(),
        crate::ContractError::TooManyItems.into()
    );

    // 2. > 10 allergies -> TooManyItems
    let mut allergies = Vec::new(&env);
    for i in 0u32..11u32 {
        allergies.push_back(String::from_str(&env, &format!("allergy_{}", i)));
    }
    let res = client.try_set_diet_plan(
        &pet_id,
        &String::from_str(&env, "Dry Food"),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &Vec::new(&env),
        &allergies,
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().unwrap(),
        crate::ContractError::TooManyItems.into()
    );

    // 3. Restriction item > 100 bytes -> InputStringTooLong
    let mut overlong_restrictions = Vec::new(&env);
    overlong_restrictions.push_back(repeat_char(&env, b'R', 101));
    let res = client.try_set_diet_plan(
        &pet_id,
        &String::from_str(&env, "Dry Food"),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &overlong_restrictions,
        &Vec::new(&env),
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().unwrap(),
        crate::ContractError::InputStringTooLong.into()
    );

    // 4. Allergy item > 100 bytes -> InputStringTooLong
    let mut overlong_allergies = Vec::new(&env);
    overlong_allergies.push_back(repeat_char(&env, b'A', 101));
    let res = client.try_set_diet_plan(
        &pet_id,
        &String::from_str(&env, "Dry Food"),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &Vec::new(&env),
        &overlong_allergies,
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().unwrap(),
        crate::ContractError::InputStringTooLong.into()
    );

    // 5. Overlong food_type -> InputStringTooLong
    let res = client.try_set_diet_plan(
        &pet_id,
        &repeat_char(&env, b'F', 101),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "Twice daily"),
        &300u32,
        &600u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().unwrap(),
        crate::ContractError::InputStringTooLong.into()
    );
}
