// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

//! This module provides mock dbreader for tests.

use crate::{DbReader, DbWriter};
use anyhow::{anyhow, Result};
use aptos_types::{
    account_address::AccountAddress,
    account_config::AccountResource,
    account_state::AccountState,
    state_store::{state_key::StateKey, state_value::StateValue},
};
use move_core_types::move_resource::MoveResource;

/// This is a mock of the DbReaderWriter in tests.
pub struct MockDbReaderWriter;

impl DbReader for MockDbReaderWriter {
    fn get_latest_state_value(&self, state_key: StateKey) -> Result<Option<StateValue>> {
        match state_key {
            StateKey::AccessPath(access_path) => {
                let account_state = get_mock_account_state();
                Ok(account_state
                    .get(&access_path.path)
                    .cloned()
                    .map(StateValue::from))
            }
            _ => Err(anyhow!("Not supported state key type {:?}", state_key)),
        }
    }
}

fn get_mock_account_state() -> AccountState {
    let account_resource = AccountResource::new(0, vec![], AccountAddress::random());

    let mut account_state = AccountState::default();
    account_state.insert(
        AccountResource::resource_path(),
        bcs::to_bytes(&account_resource).unwrap(),
    );
    account_state
}

impl DbWriter for MockDbReaderWriter {}
