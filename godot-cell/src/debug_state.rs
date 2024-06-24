#![cfg(debug_assertions)]
use std::{backtrace::Backtrace, collections::HashMap};

#[derive(Debug)]
pub(crate) struct DebugState {
    shared_borrow_count: u64,
    shared_borrows: HashMap<u64, Backtrace>,
    mutable_borrow: Option<Backtrace>,
}

impl DebugState {
    pub fn new() -> Self {
        Self {
            shared_borrow_count: 0,
            shared_borrows: HashMap::new(),
            mutable_borrow: None,
        }
    }

    pub fn track_shared_borrow(&mut self) -> u64 {
        let count = self.shared_borrow_count;
        self.shared_borrow_count += 1;

        self.shared_borrows.insert(count, Backtrace::capture());
        count
    }

    pub fn untrack_shared_borrow(&mut self, id: u64) {
        let _ = self
            .shared_borrows
            .remove(&id)
            .expect("shared borrow should be tracked");
    }

    pub fn track_mutable_borrow(&mut self, backtrace: Option<Backtrace>) {
        assert!(self.mutable_borrow.is_none());
        self.mutable_borrow = Some(match backtrace {
            Some(backtrace) => backtrace,
            None => Backtrace::capture(),
        });
    }

    pub fn untrack_mutable_borrow(&mut self) -> Backtrace {
        self.mutable_borrow.take().unwrap()
    }

    pub fn borrow_locations(&self) -> String {
        if !self.shared_borrows.is_empty() {
            let mut shared = self.shared_borrows.iter().collect::<Vec<_>>();
            shared.sort_by_key(|(i, _)| *i);
            return shared
                .into_iter()
                .rev()
                .map(|(_, b)| b.to_string())
                .collect();
        }

        if let Some(mutable) = &self.mutable_borrow {
            return mutable.to_string();
        }

        String::new()
    }
}
