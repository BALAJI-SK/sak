use anyhow::Result;
use litesvm::LiteSVM;
use solana_transaction::versioned::VersionedTransaction;
use std::collections::HashMap;
use solana_account::{AccountSharedData, ReadableAccount};

/// Result of simulating a transaction in LiteSVM.
pub struct SimulationResult {
    pub pre_balances: HashMap<String, u64>,
    pub post_balances: HashMap<String, u64>,
    pub logs: Vec<String>,
    pub success: bool,
    pub error: Option<String>,
}

pub struct Simulator {
    svm: LiteSVM,
    pre_accounts: Vec<(solana_address::Address, AccountSharedData)>,
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            svm: LiteSVM::new(),
            pre_accounts: vec![],
        }
    }

    /// Simulate a transaction using LiteSVM.
    /// Returns SimulationResult with pre/post balances and logs.
    pub fn simulate(
        &mut self,
        tx: &VersionedTransaction,
    ) -> Result<SimulationResult, String> {
        // Store pre-state (simplified - in production would snapshot all accounts)
        self.pre_accounts = vec![];

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
                    logs: sim.meta.logs.clone(),
                    success: true,
                    error: None,
                })
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }
}
