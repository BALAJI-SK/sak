use anyhow::Result;
use litesvm::LiteSVM;
use solana_message::VersionedMessage;
use solana_transaction::versioned::VersionedTransaction;
use std::collections::HashMap;
use solana_account::{Account, ReadableAccount};

/// Result of simulating a transaction in LiteSVM.
pub struct SimulationResult {
    pub pre_balances: HashMap<String, u64>,
    pub post_balances: HashMap<String, u64>,
}

pub struct Simulator {
    svm: LiteSVM,
    pre_accounts: Vec<(solana_address::Address, Account)>,
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            svm: LiteSVM::new(),
            pre_accounts: vec![],
        }
    }

    /// Create a Simulator using an existing LiteSVM instance.
    pub fn with_svm(svm: LiteSVM) -> Self {
        Self {
            svm,
            pre_accounts: vec![],
        }
    }

    /// Simulate a transaction using LiteSVM.
    /// Returns pre/post lamport balances per account key (base58).
    pub fn simulate(
        &mut self,
        tx: &VersionedTransaction,
    ) -> Result<SimulationResult, String> {
        // Step 1: snapshot pre-state from tx account keys
        let msg = match &tx.message {
            VersionedMessage::Legacy(msg) => msg,
            _ => return Err("not a legacy tx".to_string()),
        };

        self.pre_accounts = msg.account_keys
            .iter()
            .filter_map(|key| {
                self.svm.get_account(key).map(|acc| (*key, acc))
            })
            .collect();

        // Step 2: now run simulation
        let result = self.svm.simulate_transaction(tx.clone());

        match result {
            Ok(sim) => {
                let mut post_balances = HashMap::new();
                for (pubkey, account) in &sim.post_accounts {
                    post_balances.insert(pubkey.to_string(), account.lamports());
                }

                let mut pre_balances = HashMap::new();
                for (pubkey, account) in &self.pre_accounts {
                    pre_balances.insert(pubkey.to_string(), account.lamports());
                }

                Ok(SimulationResult {
                    pre_balances,
                    post_balances,
                })
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }
}
