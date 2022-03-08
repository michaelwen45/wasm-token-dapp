use crate::components::phantom_wallet::PhantomWallet;
use crate::transaction::Transaction;
use std::collections::HashMap;
use sycamore::prelude::*;

pub struct Count(pub i32);

impl Default for Count {
    fn default() -> Count {
        Count(0)
    }
}

pub type Files = HashMap<String, gloo_file::File>;
pub type FilesVec = Vec<(String, i32)>;
pub type WalletConnected = bool;

pub fn initialize_store(ctx: ScopeRef) {
    ctx.provide_context_ref(ctx.create_signal(Count::default()));
    ctx.provide_context_ref(ctx.create_signal(Files::new()));
    ctx.provide_context_ref(ctx.create_signal(FilesVec::new()));
    ctx.provide_context_ref(ctx.create_signal(Transaction::default()));
    ctx.provide_context_ref(ctx.create_signal(PhantomWallet::default()));
}
pub enum Action {
    CountIncrement(i32),
    FilesSet(web_sys::FileList),
    TransactionSet(Transaction),
    WalletSet(PhantomWallet),
}

pub fn reducer(ctx: ScopeRef, action: Action) {
    match action {
        Action::CountIncrement(increment) => {
            let count = ctx.use_context::<Signal<Count>>();
            count.set(Count(count.get().0 + increment));
        }
        Action::FilesSet(file_list) => {
            let files = ctx.use_context::<Signal<Files>>();
            let files_vec = ctx.use_context::<Signal<FilesVec>>();

            let new_files_vec = gloo_file::FileList::from(file_list).to_vec();

            files_vec.set(
                new_files_vec
                    .iter()
                    .map(|f| (f.name(), f.size() as i32))
                    .collect(),
            );

            let mut new_files = Files::new();
            new_files_vec.into_iter().for_each(|f| {
                new_files.insert(f.name(), f);
            });
            files.set(new_files);
        }
        Action::TransactionSet(transaction) => {
            let tx = ctx.use_context::<Signal<Transaction>>();
            tx.set(transaction);
        }
        Action::WalletSet(phantom_wallet) => {
            let wallet = ctx.use_context::<Signal<PhantomWallet>>();
            wallet.set(phantom_wallet);
        }
    }
}
