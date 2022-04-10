// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use crate::components::apply_chunk_output::ApplyChunkOutput;
use anyhow::Result;
use aptos_crypto::hash::TransactionAccumulatorHasher;
use aptos_logger::trace;
use aptos_state_view::StateView;
use aptos_types::{
    access_path::AccessPath,
    on_chain_config,
    on_chain_config::{access_path_for_config, ConfigurationResource, OnChainConfig, ValidatorSet},
    proof::accumulator::InMemoryAccumulator,
    state_store::state_key::StateKey,
    transaction::{Transaction, TransactionOutput},
};
use aptos_vm::VMExecutor;
use executor_types::ExecutedChunk;
use fail::fail_point;
use move_core_types::move_resource::MoveStructType;
use std::{collections::HashSet, sync::Arc};
use storage_interface::verified_state_view::{StateCache, VerifiedStateView};

pub struct ChunkOutput {
    /// Input transactions.
    pub transactions: Vec<Transaction>,
    /// Raw VM output.
    pub transaction_outputs: Vec<TransactionOutput>,
    /// Carries the frozen base state view, so all in-mem nodes involved won't drop before the
    /// execution result is processed; as well as al the accounts touched during execution, together
    /// with their proofs.
    pub state_cache: StateCache,
}

impl ChunkOutput {
    pub fn by_transaction_execution<V: VMExecutor>(
        transactions: Vec<Transaction>,
        state_view: VerifiedStateView,
    ) -> Result<Self> {
        // Warm up the state view cache by reading the configuration key, which is needed
        // later when we parse the validator set and epoch.
        // This is a Hack currently and will follow up to fix this by reading the epoch from events
        state_view.get_state_value(&StateKey::AccessPath(AccessPath::new(
            on_chain_config::config_address(),
            ConfigurationResource::struct_tag().access_vector(),
        )))?;

        let transaction_outputs = V::execute_block(transactions.clone(), &state_view)?;

        Ok(Self {
            transactions,
            transaction_outputs,
            state_cache: state_view.into_state_cache(),
        })
    }

    pub fn by_transaction_output(
        transactions_and_outputs: Vec<(Transaction, TransactionOutput)>,
        state_view: VerifiedStateView,
    ) -> Result<Self> {
        let (transactions, transaction_outputs): (Vec<_>, Vec<_>) =
            transactions_and_outputs.into_iter().unzip();

        // Warm up the state view cache by reading the configuration key, which is needed
        // later when we parse the validator set and epoch.
        state_view.get_state_value(&StateKey::AccessPath(AccessPath::new(
            on_chain_config::config_address(),
            ConfigurationResource::struct_tag().access_vector(),
        )))?;
        state_view.get_state_value(&StateKey::AccessPath(AccessPath::new(
            on_chain_config::config_address(),
            access_path_for_config(ValidatorSet::CONFIG_ID).path,
        )))?;

        // collect all accounts touched and dedup
        let access_paths = transaction_outputs
            .iter()
            .flat_map(|o| o.write_set())
            .collect::<HashSet<_>>();

        // prime the state cache by fetching all touched accounts
        // TODO: add concurrency
        access_paths
            .iter()
            .map(|(key, _)| state_view.get_state_value(key))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            transactions,
            transaction_outputs,
            state_cache: state_view.into_state_cache(),
        })
    }

    pub fn apply_to_ledger(
        self,
        base_accumulator: &Arc<InMemoryAccumulator<TransactionAccumulatorHasher>>,
    ) -> Result<(ExecutedChunk, Vec<Transaction>, Vec<Transaction>)> {
        fail_point!("executor::vm_execute_chunk", |_| {
            Err(anyhow::anyhow!("Injected error in apply_to_ledger."))
        });
        ApplyChunkOutput::apply(self, base_accumulator)
    }

    pub fn trace_log_transaction_status(&self) {
        let status: Vec<_> = self
            .transaction_outputs
            .iter()
            .map(TransactionOutput::status)
            .cloned()
            .collect();

        if !status.is_empty() {
            trace!("Execution status: {:?}", status);
        }
    }
}
