use crate::{ContractError, Gender, KoraContract, KoraContractClient, PrivacyLevel, Species};
use soroban_sdk::{testutils::Address as _, Address, Env, Error, String, Vec};

fn setup(env: &Env) -> (KoraContractClient, Address) {
    let contract_id = env.register_contract(None, KoraContract);
    let client = KoraContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.init_admin(&admin);
    (client, admin)
}

fn register(env: &Env, client: &KoraContractClient, name: &str, species: &Species, breed: &str) -> u64 {
    let owner = Address::generate(env);
    client.register_pet(
        &owner,
        &String::from_str(env, name),
        &String::from_str(env, "2020-01-01"),
        &Gender::Male,
        species,
        &String::from_str(env, breed),
        &String::from_str(env, "Brown"),
        &10,
        &None,
        &PrivacyLevel::Public,
    )
}

// --- Name validation ---

#[test]
fn test_valid_name_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let id = register(&env, &client, "Buddy-2", &Species::Dog, "Labrador");
    assert!(id > 0);
}

#[test]
#[should_panic]
fn test_empty_name_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    register(&env, &client, "", &Species::Dog, "Labrador");
}

#[test]
#[should_panic]
fn test_name_too_long_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    // 65 chars
    register(&env, &client, "AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDDEEEEEEEEEEFFFFF123456789012", &Species::Dog, "Labrador");
}

#[test]
#[should_panic]
fn test_name_invalid_chars_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    register(&env, &client, "Buddy@!", &Species::Dog, "Labrador");
}

#[test]
fn test_name_max_length_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    // exactly 64 chars
    let id = register(&env, &client, "AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDDEEEEEEEEEEFFFFF1234567890", &Species::Dog, "Labrador");
    assert!(id > 0);
}

// --- Breed whitelist ---

#[test]
fn test_breed_accepted_when_no_whitelist() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let id = register(&env, &client, "Rex", &Species::Dog, "AnythingGoes");
    assert!(id > 0);
}

#[test]
fn test_breed_accepted_when_in_whitelist() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);

    let mut breeds = Vec::new(&env);
    breeds.push_back(String::from_str(&env, "Labrador"));
    breeds.push_back(String::from_str(&env, "Poodle"));
    client.set_breed_list(&admin, &Species::Dog, &breeds);

    let id = register(&env, &client, "Rex", &Species::Dog, "Labrador");
    assert!(id > 0);
}

#[test]
#[should_panic]
fn test_breed_rejected_when_not_in_whitelist() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);

    let mut breeds = Vec::new(&env);
    breeds.push_back(String::from_str(&env, "Labrador"));
    client.set_breed_list(&admin, &Species::Dog, &breeds);

    register(&env, &client, "Rex", &Species::Dog, "Poodle");
}

#[test]
fn test_breed_whitelist_is_species_specific() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);

    // Set dog whitelist only
    let mut dog_breeds = Vec::new(&env);
    dog_breeds.push_back(String::from_str(&env, "Labrador"));
    client.set_breed_list(&admin, &Species::Dog, &dog_breeds);

    // Cat has no whitelist — any breed accepted
    let id = register(&env, &client, "Whiskers", &Species::Cat, "AnyCatBreed");
    assert!(id > 0);
}

#[test]
fn test_get_breed_list_returns_set_list() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);

    let mut breeds = Vec::new(&env);
    breeds.push_back(String::from_str(&env, "Persian"));
    breeds.push_back(String::from_str(&env, "Siamese"));
    client.set_breed_list(&admin, &Species::Cat, &breeds);

    let result = client.get_breed_list(&Species::Cat);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_clear_breed_list_allows_any_breed() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin) = setup(&env);

    let mut breeds = Vec::new(&env);
    breeds.push_back(String::from_str(&env, "Labrador"));
    client.set_breed_list(&admin, &Species::Dog, &breeds);

    // Clear by passing empty vec
    client.set_breed_list(&admin, &Species::Dog, &Vec::new(&env));

    // Now any breed is accepted
    let id = register(&env, &client, "Rex", &Species::Dog, "Poodle");
    assert!(id > 0);
}

// --- Issue #195: Validate color length and positive weight in register_pet ---

#[test]
fn test_register_pet_empty_color_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let owner = Address::generate(&env);

    let result = client.try_register_pet(
        &owner,
        &String::from_str(&env, "Buddy"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, ""),
        &10u32,
        &None,
        &PrivacyLevel::Public,
    );

    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::InvalidInput as u32,
        )))
    );
}

#[test]
fn test_register_pet_color_too_long_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let owner = Address::generate(&env);

    // 101 characters exceeds MAX_STR_SHORT (100)
    let long_color = "a".repeat(101);
    let result = client.try_register_pet(
        &owner,
        &String::from_str(&env, "Buddy"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, &long_color),
        &10u32,
        &None,
        &PrivacyLevel::Public,
    );

    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::InvalidInput as u32,
        )))
    );
}

#[test]
fn test_register_pet_zero_weight_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let owner = Address::generate(&env);

    let result = client.try_register_pet(
        &owner,
        &String::from_str(&env, "Buddy"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Brown"),
        &0u32,
        &None,
        &PrivacyLevel::Public,
    );

    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::InvalidInput as u32,
        )))
    );
}

#[test]
fn test_register_pet_valid_color_and_weight_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let owner = Address::generate(&env);

    // Exactly 1 char color
    let id1 = client.register_pet(
        &owner,
        &String::from_str(&env, "BuddyOne"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "B"),
        &1u32,
        &None,
        &PrivacyLevel::Public,
    );
    assert!(id1 > 0);

    // Exactly 100 char color
    let max_color = "c".repeat(100);
    let id2 = client.register_pet(
        &owner,
        &String::from_str(&env, "BuddyTwo"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, &max_color),
        &50u32,
        &None,
        &PrivacyLevel::Public,
    );
    assert!(id2 > 0);
}

// --- Issue #196: Maintain species indexing and count upon species update in update_pet_profile ---

#[test]
fn test_update_pet_species_updates_species_counts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let owner = Address::generate(&env);

    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Milo"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Golden"),
        &25u32,
        &None,
        &PrivacyLevel::Public,
    );

    let dog_str = String::from_str(&env, "Dog");
    let cat_str = String::from_str(&env, "Cat");

    assert_eq!(client.get_species_count(&dog_str), 1);
    assert_eq!(client.get_species_count(&cat_str), 0);

    // Update species from Dog to Cat
    let success = client.update_pet_profile(
        &pet_id,
        &String::from_str(&env, "Milo"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Cat,
        &String::from_str(&env, "Persian"),
        &String::from_str(&env, "White"),
        &10u32,
        &None,
        &PrivacyLevel::Public,
    );
    assert!(success);

    // Verify species counters reflect the change
    assert_eq!(client.get_species_count(&dog_str), 0);
    assert_eq!(client.get_species_count(&cat_str), 1);
}

// --- Issue #197: Return ContractError::PetNotFound instead of silent boolean false in update_pet_profile ---

#[test]
fn test_update_pet_profile_nonexistent_pet_returns_pet_not_found() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);

    let result = client.try_update_pet_profile(
        &999999u64,
        &String::from_str(&env, "Ghost"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Brown"),
        &15u32,
        &None,
        &PrivacyLevel::Public,
    );

    assert_eq!(
        result,
        Err(Ok(Error::from_contract_error(
            ContractError::PetNotFound as u32,
        )))
    );
}

#[test]
fn test_update_pet_profile_success_returns_true() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = setup(&env);
    let owner = Address::generate(&env);

    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Bella"),
        &String::from_str(&env, "2021-05-10"),
        &Gender::Female,
        &Species::Dog,
        &String::from_str(&env, "Beagle"),
        &String::from_str(&env, "Tricolor"),
        &12u32,
        &None,
        &PrivacyLevel::Public,
    );

    let updated = client.update_pet_profile(
        &pet_id,
        &String::from_str(&env, "BellaUpdated"),
        &String::from_str(&env, "2021-05-10"),
        &Gender::Female,
        &Species::Dog,
        &String::from_str(&env, "Beagle"),
        &String::from_str(&env, "Tricolor"),
        &13u32,
        &None,
        &PrivacyLevel::Public,
    );

    assert!(updated);
}
