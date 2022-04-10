// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use anyhow::Result;
use aptos_config::config::{RocksdbConfig, NO_OP_STORAGE_PRUNER_CONFIG};
use aptos_logger::info;
use aptosdb::AptosDB;
use std::path::PathBuf;
use storage_interface::{get_state_value_resolver_for_latest_version, DbReader};

use aptos_types::{
    account_address::AccountAddress, account_state::AccountState,
    account_state_view::AccountStateView,
};
use std::{convert::TryFrom, sync::Arc};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(long, parse(from_os_str))]
    db: PathBuf,

    #[structopt(subcommand)] // Note that we mark a field as a subcommand
    cmd: Option<Command>,
}

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(name = "list-txns")]
    ListTXNs,
    #[structopt(name = "print-txn")]
    PrintTXN { version: u64 },
    #[structopt(name = "print-account")]
    PrintAccount {
        #[structopt(parse(try_from_str))]
        address: AccountAddress,
    },
    #[structopt(name = "list-accounts")]
    ListAccounts,
}

/// Print out latest information stored in the DB.
fn print_head(db: &AptosDB) -> Result<()> {
    let si = db
        .get_startup_info()
        .expect("Can't get startup info")
        .expect("StartupInfo is empty, database is empty.");

    let version = si.latest_ledger_info.ledger_info().version();
    info!("Version: {}", version);

    info!(
        "The latest ledger info: {}",
        si.latest_ledger_info.ledger_info()
    );

    info!("Signatures: {:?}", si.latest_ledger_info.signatures());

    info!("Current EpochState: {}", si.get_epoch_state());

    let backup = db.get_backup_handler();
    let iter = backup.get_account_iter(version)?;
    let num_account_state = iter.count();
    info!("Total Accounts: {}", num_account_state);

    print_txn(db, version);

    Ok(())
}

fn print_txn(db: &AptosDB, version: u64) {
    let tx_list = db
        .get_transactions(version, 1, version, false)
        .expect("Unable to load latest TXN");
    let tx = tx_list.transactions.first().expect("Got empty txn list.");
    println!(
        "Transaction {}: {}",
        version,
        tx.format_for_client(|_| "Unexpected script in aptos-framework".to_string())
    );
}

fn print_account(db: Arc<AptosDB>, addr: AccountAddress) {
    let account_state_view =
        AccountStateView::new(&addr, get_state_value_resolver_for_latest_version(db));
    match account_state_view.get_account_resource() {
        Ok(r) => {
            if let Some(account_resource) = r {
                println!("Account {}: {:?}", addr, account_resource)
            } else {
                info!("Account {} has no account resource.", addr)
            }
        }

        Err(e) => info!(
            "Account {} exists, but have no AccountResource: {}.",
            addr, e
        ),
    }
}

fn list_txns(db: &AptosDB) {
    let version = db
        .get_latest_version()
        .expect("Unable to get latest version");
    let backup = db.get_backup_handler();
    let iter = backup
        .get_transaction_iter(0, version as usize)
        .expect("Unable to get txn iter");
    for (v, tx) in iter.enumerate() {
        println!(
            "TXN {}: {}",
            v,
            tx.expect("Unable to read TX")
                .0
                .format_for_client(|_| "Unexpected script in aptos-framework".to_string())
        );
    }
}

fn list_accounts(db: &AptosDB) {
    let version = db
        .get_latest_version()
        .expect("Unable to get latest version");
    let backup = db.get_backup_handler();
    let iter = backup
        .get_account_iter(version)
        .expect("Unagle to get account iter");
    let mut num_account = 0;
    for res in iter {
        match res {
            Ok((_, blob)) => {
                let accs =
                    AccountState::try_from(&blob.value).expect("Failed to read AccountState");
                let addr = accs
                    .get_account_address()
                    .expect("Could not get address from state");
                match addr {
                    Some(x) => {
                        num_account += 1;
                        println!("Address: {:?}", x);
                    }
                    None => println!("Skipping: No address for AccountState: {:?}", accs),
                }
            }
            Err(x) => println!("Got err iterating through AccountStateBlobs {:?}", x),
        }
    }
    info!("Total Accounts: {}", num_account);
}

fn main() {
    ::aptos_logger::Logger::builder().build();

    let opt = Opt::from_args();

    let p = opt.db.as_path();

    if !p.is_dir() {
        info!("Invalid Directory {:?}!", p);
        std::process::exit(-1);
    }

    let log_dir = tempfile::tempdir().expect("Unable to get temp dir");
    info!("Opening DB at: {:?}, log at {:?}", p, log_dir.path());

    let db = AptosDB::open(
        p,
        true,                        /* readonly */
        NO_OP_STORAGE_PRUNER_CONFIG, /* pruner config */
        RocksdbConfig::default(),
    )
    .expect("Unable to open AptosDB");
    info!("DB opened successfully.");

    if let Some(cmd) = opt.cmd {
        match cmd {
            Command::ListTXNs => {
                list_txns(&db);
            }
            Command::PrintTXN { version } => {
                print_txn(&db, version);
            }
            Command::PrintAccount { address } => {
                print_account(Arc::new(db), address);
            }
            Command::ListAccounts => {
                list_accounts(&db);
            }
        }
    } else {
        print_head(&db).expect("Unable to read information from DB");

        Opt::clap().print_help().unwrap();
        println!();
    }
}
