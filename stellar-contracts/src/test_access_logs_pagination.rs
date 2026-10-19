use crate::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Env;

#[test]
fn test_get_access_logs_pagination() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Rex"),
        &String::from_str(&env, "2021-06-15"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Black"),
        &30u32,
        &None,
        &PrivacyLevel::Public,
    );

    // Request first page: offset=0, limit=10
    let page1 = client.get_access_logs(&pet_id, &owner, &0u64, &10u32);
    // No logs yet, but call should succeed
    assert_eq!(page1.len(), 0);

    // Request with limit larger than max (50) should still succeed and clamp
    let page2 = client.get_access_logs(&pet_id, &owner, &0u64, &100u32);
    assert_eq!(page2.len(), 0);
}

#[test]
fn test_get_access_logs_offset_beyond_end() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Rex"),
        &String::from_str(&env, "2021-06-15"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Black"),
        &30u32,
        &None,
        &PrivacyLevel::Public,
    );

    // Offset beyond end should return empty, not panic
    let page = client.get_access_logs(&pet_id, &owner, &9999u64, &10u32);
    assert_eq!(page.len(), 0);
}
