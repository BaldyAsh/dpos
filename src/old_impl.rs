use std::cmp;
use std::collections::HashMap;

use super::Address;
use super::Amount;
use super::Index;
use super::SHARE;

type Hash = u128;

// Maximum number of reward 'events' that can be processed in one request to prevent to prevent excessive consumption of resources
const INDEX_MAX_DELTA: u32 = 1000;

pub struct Hasher {}

impl Hasher {
    fn hash(index: Index, address: Address) -> u128 {
        index as u128 + address
    }
}

pub struct User {
    // User address
    pub address: Address,
    // User balance
    pub balance: Amount,
}

pub struct Validator {
    // Total token balance for that validator
    pub total_balance: Amount,
    // Current reward index for that validator (some sort of timestamp or reward-block-number), incremented
    pub current_index: Index,
    // Total tokens support for some reward by its index
    pub total_support: HashMap<Index, Amount>,
    // Reward by its index
    pub reward: HashMap<Index, Amount>,
    // User support deposited at some reward index - Hash(reward_index, user_address)
    pub user_support: HashMap<Hash, Amount>,
    // User support where the user has money
    pub user_support_indexes: HashMap<Address, Vec<Index>>,
}

trait Democracy {
    fn vote(&mut self, user: &mut User, amount: Amount) -> (Index, Amount);
}

trait RewardSharing {
    fn append_reward(&mut self, reward: Amount);
    fn try_withdraw_with_rewards(
        &mut self,
        user: &mut User,
        from_index: Index,
        amount: Amount,
    ) -> Option<(Index, Amount)>;
}

impl Democracy for Validator {
    fn vote(&mut self, user: &mut User, amount: Amount) -> (Index, Amount) {
        // Update total balance
        self.total_balance += amount;

        // Update total support at current index
        let update = match self.total_support.get(&self.current_index) {
            Some(supported) => supported + amount,
            None => amount,
        };
        self.total_support.insert(self.current_index, update);

        // Get hash from address and current index
        let hash = Hasher::hash(self.current_index, user.address);

        // Update user balance at current index
        let update = match self.user_support.get(&hash) {
            Some(supported) => supported + amount,
            None => amount,
        };

        self.user_support.insert(hash, update);

        user.balance -= amount;

        // Return current index and updated support amount for user
        (self.current_index, update)
    }
}

impl RewardSharing for Validator {
    fn append_reward(&mut self, reward: Amount) {
        // Insert new index support - its Amount is current total balance
        self.total_support.insert(
            self.current_index + 1,
            self.total_support
                .get(&self.current_index)
                .cloned()
                .unwrap_or(0),
        );

        // Update index
        self.current_index += 1;

        // Update total balance
        self.total_balance += reward;
    }

    fn try_withdraw_with_rewards(
        &mut self,
        user: &mut User,
        from_index: Index,
        amount: Amount,
    ) -> Option<(Index, Amount)> {
        // Get hash from address and current index
        let hash = Hasher::hash(from_index, user.address);

        // Get user support balance at index
        let supported = self.user_support.get(&hash).cloned().unwrap();

        // Accumulate rewards until the current or max possible index
        let max_index = from_index + INDEX_MAX_DELTA;
        let end_index = cmp::max(max_index, self.current_index);

        let mut reward = 0;
        for i in from_index..end_index {
            let user_share = amount / self.total_support.get(&i)?;
            reward += self.reward.get(&i).cloned().unwrap_or(0) * SHARE * user_share / 100;
        }

        // Update supporter balance at index: subtract provided amount
        self.user_support.insert(hash, supported - amount);

        // Make a decision - how much to withdraw depending on processed indexes length
        if end_index < self.current_index {
            // If there are rewards left after the last processed index -
            // place the provided amount to the upper bound index and withdraw only reward
            let hash = Hasher::hash(end_index, user.address);

            let new_balance = match self.user_support.get(&hash) {
                Some(balance) => balance + amount,
                None => amount,
            };

            self.user_support.insert(hash, new_balance);

            // Send only the reward
            self.total_balance -= reward;
            user.balance += reward;

            // Return updated upper bound index
            Some((end_index, new_balance))
        } else {
            // Withdraw all
            self.total_balance -= amount + reward;
            user.balance += amount + reward;

            // Return none - everything has been withdrawn
            None
        }
    }
}
