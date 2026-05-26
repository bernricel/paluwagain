#![cfg(test)]

use super::{PaluwaGainContract, PaluwaGainContractClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{token, vec, Address, Env};
use token::{StellarAssetClient, TokenClient};

struct TestSetup<'a> {
    env: Env,
    client: PaluwaGainContractClient<'a>,
    token: TokenClient<'a>,
    admin: Address,
    member1: Address,
    member2: Address,
    member3: Address,
    contribution: i128,
}

fn setup<'a>() -> TestSetup<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let member1 = Address::generate(&env);
    let member2 = Address::generate(&env);
    let member3 = Address::generate(&env);

    let contract_id = env.register(PaluwaGainContract, ());
    let client = PaluwaGainContractClient::new(&env, &contract_id);

    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_address = sac.address();
    let token = TokenClient::new(&env, &token_address);
    let token_admin = StellarAssetClient::new(&env, &token_address);

    let contribution: i128 = 500;
    let starting_balance: i128 = 2_000;
    token_admin.mint(&member1, &starting_balance);
    token_admin.mint(&member2, &starting_balance);
    token_admin.mint(&member3, &starting_balance);

    let members = vec![&env, member1.clone(), member2.clone(), member3.clone()];
    client.initialize(&admin, &token_address, &contribution, &members);

    TestSetup {
        env,
        client,
        token,
        admin,
        member1,
        member2,
        member3,
        contribution,
    }
}

#[test]
fn happy_path_releases_first_round_payout() {
    let setup = setup();

    setup.client.deposit(&setup.member1);
    setup.client.deposit(&setup.member2);
    setup.client.deposit(&setup.member3);

    let receiver = setup.client.release_payout();

    assert_eq!(receiver, setup.member1);
    assert_eq!(setup.token.balance(&setup.member1), 3_000);
    assert_eq!(setup.client.get_current_round(), 1);
}

#[test]
#[should_panic(expected = "member already paid this round")]
fn duplicate_deposit_fails() {
    let setup = setup();

    setup.client.deposit(&setup.member1);
    setup.client.deposit(&setup.member1);
}

#[test]
fn state_verification_after_one_deposit() {
    let setup = setup();

    setup.client.deposit(&setup.member2);

    assert_eq!(setup.client.has_paid(&setup.member2, &0u32), true);
    assert_eq!(setup.client.get_paid_count(&0u32), 1);
    assert_eq!(setup.token.balance(&setup.member2), 1_500);
}

#[test]
#[should_panic(expected = "not all members have paid")]
fn release_before_all_members_paid_fails() {
    let setup = setup();

    setup.client.deposit(&setup.member1);
    setup.client.deposit(&setup.member2);

    setup.client.release_payout();
}

#[test]
fn status_returns_dashboard_values() {
    let setup = setup();

    setup.client.deposit(&setup.member1);
    let status = setup.client.get_status();

    assert_eq!(status.0, 0);
    assert_eq!(status.1, 1);
    assert_eq!(status.2, setup.member1);
    assert_eq!(status.3, setup.contribution);
    assert_eq!(status.4, 3);
}
