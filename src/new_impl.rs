use super::Address;
use super::Amount;
use super::Index;
use super::SHARE;
use std::collections::HashMap;

pub struct Vote {
    // The number of rewards that are already on the account at the time of voting
    pub first_reward_id: Index,
    // Vote amount
    pub amount: Amount,
    // Indicates that the reward has been withdrawn for a given vote and it remains to close this vote
    pub reward_taken: bool,
}

pub struct User {
    // User address
    pub address: Address,
    // User balance
    pub balance: Amount,
}

pub struct Validator {
    // Users votes by their addresses
    pub votes: HashMap<Address, Vote>,
    // Delegated balance on that account
    pub total_delegated: Amount,
    // Total balance on that account (delegated + rewarded)
    pub total_balance: Amount,
    // Number of rewards for that validator
    pub rewards_count: Index,
    // The average reward value available for withdrawal by delegates.
    // reward_for_user = (delegated_by_user / total_delegated) * (rewards_count - user_vote_time_rewards_count) * reward_to_share
    pub reward_to_share: Amount,
}

trait Democracy {
    fn vote(&mut self, user: &mut User, amount: Amount);
    fn unvote(&mut self, user: &mut User);
}

trait RewardSharing {
    fn append_reward(&mut self, reward: Amount);
    fn send_rewards(&mut self, user: &mut User);
}

impl Democracy for Validator {
    fn vote(&mut self, user: &mut User, amount: Amount) {
        // First check that user has no votes (her previous vote and reward for it has been withdrawn)
        if let Some(prev_vote) = self.votes.get(&user.address) {
            if prev_vote.amount > 0 || !prev_vote.reward_taken {
                panic!("Get reward and unvote before revoting");
            }
        }

        // Insert new vote
        self.votes.insert(
            user.address,
            Vote {
                first_reward_id: self.rewards_count,
                amount,
                reward_taken: false,
            },
        );

        // Update balances: user, delegated, total
        user.balance -= amount;
        self.total_delegated += amount;
        self.total_balance += amount;
    }

    fn unvote(&mut self, user: &mut User) {
        // Check that vote exists
        let vote = self.votes.get(&user.address);
        if vote.is_none() {
            panic!("Nothing to unvote")
        }

        let vote = vote.unwrap();

        // Vote amount must not be zero and its reward must be withdrawn
        if vote.amount == 0 || !vote.reward_taken {
            panic!("Make sure that the vote exists and the reward has been withdrawn");
        }

        // Update balances: user, delegated and total
        user.balance += vote.amount;
        self.total_delegated -= vote.amount;
        self.total_balance -= vote.amount;

        // Delete vote
        self.votes.remove(&user.address);
    }
}

impl RewardSharing for Validator {
    fn append_reward(&mut self, reward: Amount) {
        // Update total balance
        self.total_balance += reward;

        // Update passed rewards count
        self.rewards_count += 1;

        // Calculate new value for a reward to share with users
        let medium = (self.reward_to_share + reward) / 2;
        self.reward_to_share = SHARE * medium / 100;
    }

    fn send_rewards(&mut self, user: &mut User) {
        // Check that vote exists
        let vote = self.votes.get(&user.address);
        if vote.is_none() {
            panic!("No vote to get rewards")
        }

        let vote = vote.unwrap();

        // Vote amount must not be zero (it must not be withdrawn) and reward has not been taken
        let amount = vote.amount;
        if amount == 0 || vote.reward_taken {
            panic!("Make sure that the vote exists and the reward has not been withdrawn. If reward has been withdrawn - unvote.");
        }

        // Calculate rewards count that passed since user vote
        let first_reward_id = vote.first_reward_id;
        let rewards_passed = (self.rewards_count - first_reward_id) as u128;
        // Calculate reward
        let reward = (vote.amount / self.total_delegated) * rewards_passed * self.reward_to_share;

        // Update user and total balances
        user.balance += reward;
        self.total_balance -= reward;

        // Update vote - reward has been taken
        self.votes.insert(
            user.address,
            Vote {
                first_reward_id,
                amount,
                reward_taken: true,
            },
        );
    }
}
