//! 领域类型模块根。

pub mod checklist;
pub mod domain;

pub use checklist::{checklist_entries, ChecklistEntry};
pub use domain::{derive_verdict, ChecklistItem, Verdict};

#[cfg(test)]
mod checklist_test;
#[cfg(test)]
mod domain_test;
