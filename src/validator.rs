use chain::{Chain, ChainRequest};
use failure::{ensure, format_err};
use futures::channel::{mpsc, oneshot};
use futures::executor::block_on;
pub use hasher::Hasher;
pub use signature::Signature;
use std::cmp;
use std::collections::HashMap;
use verifier::verify_signature;

// Lets call it 'A number of reward for that validator'
pub type Index = u32;
// Value in chain tokens
pub type Value = u32;
// Address in chain
pub type Address = [u8; 20];

// Validator reward share
const VALIDATOR_SHARE: f64 = 0.3;
// Maximum number of reward 'events' that can be processed in one request to prevent to prevent excessive consumption of resources
const INDEX_MAX_DELTA: u32 = 1000;

pub struct Validator<Hash, H: Hasher<Hash>> {
    // A program that performs some hashing algorithm
    pub hasher: H,
    // Sender channel half - sends requests to chain
    pub chain_sender: mpsc::Sender<ChainRequest>,
    // Validators owner address in chain (human address)
    pub owner_address: Address,
    // This validators address in chain (program address)
    pub validator_address: Address,
    // Total token balance for that validator
    pub total_balance: Value,
    // Total reward to withdraw for validator owner
    pub total_owner_reward: Value,
    // Current reward index for that validator (some sort of timestamp or reward-block-number), incremented
    pub current_index: Index,
    // Total tokens support for some reward by its index
    pub total_support: HashMap<Index, Value>,
    // Reward by its index
    pub reward: HashMap<Index, Value>,
    // User support deposited at some reward index - Hash(reward_index, user_address)
    pub user_support: HashMap<Hash, Value>,
    // User support where the user has money
    pub user_support_indexes: HashMap<Address, Vec<Index>>,
}

impl<Hash, H> Validator<Hash, H>
where
    H: Hasher<Hash>,
{
    // Creates Validator instance
    pub fn create(chain_sender: mpsc::Sender<ChainRequest>, owner: Address) -> Self {
        Validator {
            hasher: H::default(),
            chain_sender,
            owner_address: owner,
            validator_address: Chain::generate_address(owner),
            total_balance: 0,
            total_owner_reward: 0,
            current_index: 0,
            total_support: HashMap::new(),
            reward: HashMap::new(),
            user_support: HashMap::new(),
            user_support_indexes: HashMap::new(),
        }
    }

    // Returns all support indexes for user
    pub fn get_support_indexes(&mut self, user_address: Address) -> Option<Vec<Index>> {
        self.user_support_indexes.get(user_address)
    }

    /// User can vote for that validator, providing her address, support amount and signature
    ///
    /// Returns index of the claimed reward and total value of user support for that reward
    ///
    /// # Arguments
    ///
    /// * `user_address` - User address
    /// * `amount` - Support amount
    /// * `signature` - Signature(user_address, validator_address, amount)
    ///
    pub fn vote(
        &mut self,
        user_address: Address,
        amount: Value,
        signature: Signature,
    ) -> Result<(Index, Value), failure::Error> {
        // Verify user signature
        let mut packed_bits = vec![];
        packed_bits.extend(user_address.to_bits());
        packed_bits.extend(self.validator_address.to_bits());
        packed_bits.extend(amount.to_bits());
        verify_signature(packed_bits, user_address, signature)?;
        // Transfer funds from user to validator address
        let resp = async {
            let resp = oneshot::channel();
            self.chain_sender
                .clone()
                .send(ChainRequest::Transfer(
                    user_address,
                    self.validator_address,
                    amount,
                    resp.0,
                ))
                .await
                .expect("Dropped");
            let result = resp
                .1
                .await
                .map_err(|e| format_err!("Transfer failed: {}", e))?;
            Ok(result.unwrap())
        };
        block_on(resp)?;
        // Update total balance
        self.total_balance += amount;
        // Update total support at current index
        let update = self.total_support.get(self.current_index)? + amount;
        self.total_support.insert(self.current_index, update);
        // Update user balance at current index
        let mut bits = self.current_index.to_bits();
        bits.extend(user_address.to_bits());
        let hash = self.hasher.hash_bits(bits);
        let update = self.user_support.get(&hash)? + amount;
        self.user_support.insert(hash, update);
        self.user_support_indexes
            .insert(user_address, self.current_index);
        self.user_support_indexes.dedup();
        // Return current index and updated support amount for user
        Ok((self.current_index, update))
    }

    /// If validator got reward it is inserted at current reward index, index is incremented and
    /// total support amount for next reward index is copyed from current total balance
    ///
    /// # Arguments
    ///
    /// * `amount` - Reward amount
    ///
    pub fn new_reward(&mut self, amount: Value) {
        // Update owner reward
        self.total_owner_reward += amount * VALIDATOR_SHARE;
        // Insert new index support - its value is current total balance
        self.total_support.insert(
            self.current_index + 1,
            self.total_support.get(self.current_index)?,
        );
        // Update index
        self.current_index += 1;
        // Update total balance
        self.total_balance += amount;
    }

    /// User can try to withdraw her supply at some reward index and rewards for it.
    ///
    /// That the reward for provided amount will be accrued for all ongoing rewards indexes until some maximum possible index.
    /// After that this amount will be subtracted at provided index.
    ///
    /// If maximum possible index is the last (current) index - user will withdraw requested amount plus all rewards for it.
    /// Otherwise she will receive only rewards and specified amount will be placed at next index after maximum possible,
    /// so user will have an opportunity to finish the process later.
    ///
    /// This is done so that the complexity of this operation has a certain constant upper limit and to avoid excessive consumption of computing power.
    /// If process has not been finished this operation will return index of the position after maximum possible and also updated user balance
    /// for that index.
    ///
    /// # Arguments
    ///
    /// * `user_address` - User address
    /// * `from_index` - Index from which to start a withdrawing of the specified amount
    /// * `amount` - Amount to withdraw
    /// * `signature` - Signature(validator_address, user_address, from_index, amount)
    ///
    pub fn user_withdraw_amount_with_reward(
        &mut self,
        user_address: Address,
        from_index: Index,
        amount: Value,
        signature: Signature,
    ) -> Result<Option<(Index, Value)>, failure::Error> {
        // Verify user signature
        let mut packed_bits = vec![];
        packed_bits.extend(self.validator_address.to_bits());
        packed_bits.extend(user_address.to_bits());
        packed_bits.extend(from_index.to_bits());
        packed_bits.extend(amount.to_bits());
        verify_signature(packed_bits, user_address, signature)?;
        // Get user support balance at index
        let mut bits = from_index.to_bits();
        bits.extend(user_address.to_bits());
        let hash = self.hasher.hash_bits(bits);
        let supported = self.user_support.get(hash)?;
        ensure!(amount <= supported, "Wrong amount");
        // Accumulate rewards until the current or max possible index
        let max_index = from_index + INDEX_MAX_DELTA;
        let end_index = cmp::max(max_index, self.current_index);
        let reward = 0;
        for i in from_index..end_index {
            let user_share = amount / self.total_support.get(i)?;
            reward += self.reward.get(i)? * (1 - VALIDATOR_SHARE) * user_share;
        }
        // Update supporter balance at index: subtract provided amount
        self.user_support.insert(hash, supported - amount);
        // If supported is eq to specified amount - remove provided index from possible withdraw indexes for user
        if supported == amount {
            self.user_support_indexes.remove(from_index);
        }
        if end_index < self.current_index {
            // If there are rewards left after the last processed index -
            // place the provided amount to the upper bound index and withdraw only reward
            let mut bits = end_index.to_bits();
            bits.extend(user_address.to_bits());
            let hash = self.hasher.hash_bits(bits);
            let new_balance = self.user_support.get(hash)? + amount;
            self.user_support.insert(hash, new_balance);
            // Send only the reward
            self.total_balance -= reward;
            let resp = async {
                let resp = oneshot::channel();
                self.chain_sender
                    .clone()
                    .send(ChainRequest::Transfer(
                        self.validator_address,
                        user_address,
                        reward,
                        resp.0,
                    ))
                    .await
                    .expect("Dropped");
                let result = resp
                    .1
                    .await
                    .map_err(|e| format_err!("Transfer failed: {}", e))?;
                Ok(result.unwrap())
            };
            block_on(resp)?;
            // Return updated upper bound index
            Ok(Some((end_index, new_balance)))
        } else {
            // Withdraw all
            self.total_balance -= amount + reward;
            let resp = async {
                let resp = oneshot::channel();
                self.chain_sender
                    .clone()
                    .send(ChainRequest::Transfer(
                        self.validator_address,
                        user_address,
                        amount + reward,
                        resp.0,
                    ))
                    .await
                    .expect("Dropped");
                let result = resp
                    .1
                    .await
                    .map_err(|e| format_err!("Transfer failed: {}", e))?;
                Ok(result.unwrap())
            };
            block_on(resp)?;
            // Return none - everything has been withdrawn
            Ok(None)
        }
    }

    /// Owner can try to withdraw her rewards share.
    ///
    /// # Arguments
    ///
    /// * `amount` - Amount to withdraw
    /// * `signature` - Signature(validator_address, owner_address, amount)
    ///
    pub fn owner_withdraw_reward(
        &mut self,
        amount: Value,
        signature: Signature,
    ) -> Result<(), failure::Error> {
        ensure!(self.total_owner_reward >= amount, "Insufficient funds");
        // Verify owner signature
        let mut packed_bits = vec![];
        packed_bits.extend(self.validator_address.to_bits());
        packed_bits.extend(self.owner_address.to_bits());
        packed_bits.extend(amount.to_bits());
        verify_signature(packed_bits, self.owner_address, signature)?;
        // Withdraw reward
        self.total_owner_reward -= amount;
        self.total_balance -= amount;
        // Send reward
        let resp = async {
            let resp = oneshot::channel();
            self.chain_sender
                .clone()
                .send(ChainRequest::Transfer(
                    self.validator_address,
                    self.owner_address,
                    amount,
                    resp.0,
                ))
                .await
                .expect("Dropped");
            let result = resp
                .1
                .await
                .map_err(|e| format_err!("Transfer failed: {}", e))?;
            Ok(result.unwrap())
        };
        block_on(resp)?;
    }
}
