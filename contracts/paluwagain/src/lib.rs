#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Vec};

// Storage keys used by the PaluwaGain contract.
// These make the contract state explicit and easy to inspect during demos.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Initialized,
    Admin,
    Token,
    Members,
    Contribution,
    CurrentRound,
    PaidCount(u32),
    Paid(Address, u32),
}

#[contract]
pub struct PaluwaGainContract;

#[contractimpl]
impl PaluwaGainContract {
    /// Initializes one paluwagan cycle.
    ///
    /// MVP mapping:
    /// Organizer action -> save members, token, contribution amount, and payout order on-chain.
    /// The payout order is the same order as the `members` vector.
    pub fn initialize(env: Env, admin: Address, token: Address, contribution: i128, members: Vec<Address>) {
        admin.require_auth();

        if Self::is_initialized(env.clone()) {
            panic!("already initialized");
        }
        if contribution <= 0 {
            panic!("contribution must be positive");
        }
        if members.len() == 0 {
            panic!("members required");
        }

        env.storage().persistent().set(&DataKey::Initialized, &true);
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Token, &token);
        env.storage().persistent().set(&DataKey::Contribution, &contribution);
        env.storage().persistent().set(&DataKey::Members, &members);
        env.storage().persistent().set(&DataKey::CurrentRound, &0u32);
        env.storage().persistent().set(&DataKey::PaidCount(0u32), &0u32);
    }

    /// Lets one paluwagan member pay the required contribution for the current round.
    ///
    /// MVP mapping:
    /// Member action -> token transfer into the contract -> payment status is recorded on-chain.
    pub fn deposit(env: Env, member: Address) {
        member.require_auth();
        Self::require_initialized(&env);
        Self::require_member(&env, &member);

        let round = Self::get_current_round(env.clone());
        let paid_key = DataKey::Paid(member.clone(), round);
        let already_paid: bool = env
            .storage()
            .persistent()
            .get(&paid_key)
            .unwrap_or(false);

        if already_paid {
            panic!("member already paid this round");
        }

        let token_address = Self::get_token(env.clone());
        let contribution = Self::get_contribution(env.clone());
        let token_client = token::Client::new(&env, &token_address);

        // Move the member's contribution into this contract.
        // In production, the token can be a Stellar Asset Contract such as USDC on Stellar.
        token_client.transfer(&member, &env.current_contract_address(), &contribution);

        let count_key = DataKey::PaidCount(round);
        let current_count: u32 = env
            .storage()
            .persistent()
            .get(&count_key)
            .unwrap_or(0u32);

        env.storage().persistent().set(&paid_key, &true);
        env.storage()
            .persistent()
            .set(&count_key, &(current_count + 1));
    }

    /// Releases the pooled paluwagan payout to the scheduled receiver.
    ///
    /// MVP mapping:
    /// Anyone calls release after all deposits -> contract checks completion -> funds move to receiver.
    pub fn release_payout(env: Env) -> Address {
        Self::require_initialized(&env);

        let round = Self::get_current_round(env.clone());
        let members = Self::get_members(env.clone());

        if round >= members.len() {
            panic!("cycle already complete");
        }

        let paid_count = Self::get_paid_count(env.clone(), round);
        if paid_count != members.len() {
            panic!("not all members have paid");
        }

        let receiver = members.get(round).unwrap();
        let contribution = Self::get_contribution(env.clone());
        let pooled_amount = contribution * (members.len() as i128);
        let token_address = Self::get_token(env.clone());
        let token_client = token::Client::new(&env, &token_address);

        // Send the collected funds from the contract to the scheduled receiver.
        token_client.transfer(&env.current_contract_address(), &receiver, &pooled_amount);

        // Move to the next paluwagan round and initialize its paid counter.
        let next_round = round + 1;
        env.storage()
            .persistent()
            .set(&DataKey::CurrentRound, &next_round);
        env.storage()
            .persistent()
            .set(&DataKey::PaidCount(next_round), &0u32);

        receiver
    }

    /// Returns the main dashboard values for a frontend demo.
    /// Output: current round, paid count, next receiver, contribution amount, and member count.
    pub fn get_status(env: Env) -> (u32, u32, Address, i128, u32) {
        Self::require_initialized(&env);
        let round = Self::get_current_round(env.clone());
        let members = Self::get_members(env.clone());
        let next_receiver = if round < members.len() {
            members.get(round).unwrap()
        } else {
            members.get(members.len() - 1).unwrap()
        };

        (
            round,
            Self::get_paid_count(env.clone(), round),
            next_receiver,
            Self::get_contribution(env.clone()),
            members.len(),
        )
    }

    /// Checks whether a specific member has paid for a specific round.
    pub fn has_paid(env: Env, member: Address, round: u32) -> bool {
        Self::require_initialized(&env);
        env.storage()
            .persistent()
            .get(&DataKey::Paid(member, round))
            .unwrap_or(false)
    }

    /// Returns true after the contract has been initialized.
    pub fn is_initialized(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Initialized)
            .unwrap_or(false)
    }

    /// Returns the current round index.
    pub fn get_current_round(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::CurrentRound)
            .unwrap_or(0u32)
    }

    /// Returns how many members have paid in a selected round.
    pub fn get_paid_count(env: Env, round: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::PaidCount(round))
            .unwrap_or(0u32)
    }

    /// Returns the contribution required per member per round.
    pub fn get_contribution(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Contribution)
            .unwrap()
    }

    /// Returns the Stellar token contract address used for contributions and payouts.
    pub fn get_token(env: Env) -> Address {
        env.storage().persistent().get(&DataKey::Token).unwrap()
    }

    /// Returns all paluwagan members in fixed payout order.
    pub fn get_members(env: Env) -> Vec<Address> {
        env.storage().persistent().get(&DataKey::Members).unwrap()
    }

    fn require_initialized(env: &Env) {
        let initialized: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Initialized)
            .unwrap_or(false);
        if !initialized {
            panic!("not initialized");
        }
    }

    fn require_member(env: &Env, member: &Address) {
        let members: Vec<Address> = env.storage().persistent().get(&DataKey::Members).unwrap();
        let mut found = false;

        for stored_member in members.iter() {
            if stored_member == *member {
                found = true;
                break;
            }
        }

        if !found {
            panic!("not a paluwagan member");
        }
    }
}

mod test;
