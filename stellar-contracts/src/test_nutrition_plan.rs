use crate::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

fn setup() -> (Env, KoraContractClient<'static>, Address, u64) {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let contract_id = env.register(KoraContract, ());
    let client = KoraContractClient::new(&env, &contract_id);

    let pet_id = client.register_pet(
        &owner,
        &String::from_str(&env, "Buddy"),
        &String::from_str(&env, "2020-01-01"),
        &Gender::Male,
        &Species::Dog,
        &String::from_str(&env, "Labrador"),
        &String::from_str(&env, "Black"),
        &30u32,
        &None,
        &PrivacyLevel::Public,
    );

    (env, client, owner, pet_id)
}

fn ingredient(env: &Env, name: &str, calories: u32) -> Ingredient {
    Ingredient {
        name: String::from_str(env, name),
        calories,
    }
}

#[test]
fn test_add_nutrition_plan_exact_calories() {
    let (env, client, _owner, pet_id) = setup();

    let mut ingredients = Vec::new(&env);
    ingredients.push_back(ingredient(&env, "Chicken", 200));
    ingredients.push_back(ingredient(&env, "Rice", 150));
    ingredients.push_back(ingredient(&env, "Carrots", 50));

    let plan_id = client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "Balanced Meal"),
        &ingredients,
        &400u32,
    );

    let plan = client.get_nutrition_plan(&plan_id).unwrap();
    assert_eq!(plan.total_calories, 400u32);
    assert_eq!(plan.ingredients.len(), 3);
}

#[test]
fn test_add_nutrition_plan_within_tolerance() {
    let (env, client, _owner, pet_id) = setup();

    let mut ingredients = Vec::new(&env);
    ingredients.push_back(ingredient(&env, "Chicken", 200));
    ingredients.push_back(ingredient(&env, "Rice", 150));
    ingredients.push_back(ingredient(&env, "Carrots", 50));

    // Declared total is 3 kcal off the actual sum (400) → within ±5 tolerance
    let plan_id = client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "Balanced Meal"),
        &ingredients,
        &397u32,
    );

    let plan = client.get_nutrition_plan(&plan_id).unwrap();
    assert_eq!(plan.total_calories, 397u32);
}

#[test]
#[should_panic]
fn test_add_nutrition_plan_outside_tolerance() {
    let (env, client, _owner, pet_id) = setup();

    let mut ingredients = Vec::new(&env);
    ingredients.push_back(ingredient(&env, "Chicken", 200));
    ingredients.push_back(ingredient(&env, "Rice", 150));
    ingredients.push_back(ingredient(&env, "Carrots", 50));

    // Declared total is 10 kcal off the actual sum (400) → outside ±5 tolerance
    client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "Balanced Meal"),
        &ingredients,
        &410u32,
    );
}

/// Issue #296: a plan declared with 0 calories must be rejected.
#[test]
#[should_panic]
fn test_add_nutrition_plan_zero_calories_rejected() {
    let (env, client, _owner, pet_id) = setup();

    let mut ingredients = Vec::new(&env);
    ingredients.push_back(ingredient(&env, "Water", 0));

    // 0 kcal is below the minimum of 50 — must panic.
    client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "Empty Plan"),
        &ingredients,
        &0u32,
    );
}

/// Issue #296: a plan declared with calories exceeding 10,000 must be rejected.
#[test]
#[should_panic]
fn test_add_nutrition_plan_excessive_calories_rejected() {
    let (env, client, _owner, pet_id) = setup();

    let mut ingredients = Vec::new(&env);
    ingredients.push_back(ingredient(&env, "Lard", 10_001));

    // 10_001 kcal exceeds the maximum of 10,000 — must panic.
    client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "Calorie Bomb"),
        &ingredients,
        &10_001u32,
    );
}

/// Issue #296: a plan with an empty ingredient list must be rejected.
#[test]
#[should_panic]
fn test_add_nutrition_plan_empty_ingredients_rejected() {
    let (env, client, _owner, pet_id) = setup();

    let ingredients: Vec<Ingredient> = Vec::new(&env);

    client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "No Ingredients"),
        &ingredients,
        &200u32,
    );
}

/// Issue #296: a plan with more than 20 ingredients must be rejected.
#[test]
#[should_panic]
fn test_add_nutrition_plan_too_many_ingredients_rejected() {
    let (env, client, _owner, pet_id) = setup();

    // 21 ingredients each with 10 kcal = 210 total (within bounds), but count > 20.
    let mut ingredients = Vec::new(&env);
    for i in 0u32..21 {
        let name = soroban_sdk::String::from_str(&env, "item");
        let _ = i;
        ingredients.push_back(Ingredient { name, calories: 10 });
    }

    client.add_nutrition_plan(
        &pet_id,
        &String::from_str(&env, "Too Many"),
        &ingredients,
        &210u32,
    );
}

/// Issue #297: set_nutrition_version must assign monotonically sequential
/// version IDs — calling it three times must produce versions 1, 2, 3.
#[test]
fn test_nutrition_version_sequencing() {
    let (env, client, _owner, pet_id) = setup();

    let v1 = client.set_nutrition_version(
        &pet_id,
        &String::from_str(&env, "Dry"),
        &String::from_str(&env, "200g"),
        &String::from_str(&env, "twice daily"),
        &200u32,
        &400u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );

    let v2 = client.set_nutrition_version(
        &pet_id,
        &String::from_str(&env, "Wet"),
        &String::from_str(&env, "250g"),
        &String::from_str(&env, "once daily"),
        &300u32,
        &300u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );

    let v3 = client.set_nutrition_version(
        &pet_id,
        &String::from_str(&env, "Mixed"),
        &String::from_str(&env, "225g"),
        &String::from_str(&env, "twice daily"),
        &250u32,
        &500u32,
        &Vec::new(&env),
        &Vec::new(&env),
    );

    // Versions must be strictly sequential with no gaps.
    assert_eq!(v1, 1u64, "first version must be 1");
    assert_eq!(v2, 2u64, "second version must be 2");
    assert_eq!(v3, 3u64, "third version must be 3");

    // Only the latest version should be active.
    let latest = client.get_nutrition_version(&pet_id, &3u64).unwrap();
    assert!(latest.is_active);

    let old = client.get_nutrition_version(&pet_id, &1u64).unwrap();
    assert!(!old.is_active, "older versions must be inactive");
}
