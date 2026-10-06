//! [`ChangeStateStore`](workflow::state::ChangeStateStore) 的 store 适配器：
//! trait 委托胶水，零具名导出——读 / 写半边逐条委托 [`Store`] 的 change 域操
//! 作面，`StoreError` → `StoreFault` 错误面映射收本文件单点（映射语义本身归
//! store.rs 的 D2 单点，适配层不重复记录字段面）。

use workflow::state::{
    BacktrackCommand, ChangeStateRecord, ChangeStateStore, PhaseLogCommand, PhaseStartState,
    PhaseStateRecord, StepCommand, StepStateRecord, StoreFault,
};

use crate::store::{Store, StoreError};

/// 错误面映射（`Canonicalize` 为 workspace 注册表域故障，change 域不可达，
/// 防御式收敛 `Db`）。
fn fault(error: StoreError) -> StoreFault {
    match error {
        StoreError::Db(msg) => StoreFault::Db(msg),
        StoreError::Canonicalize(msg) => StoreFault::Db(msg),
        StoreError::Conflict(msg) => StoreFault::Conflict(msg),
        StoreError::NotFound(msg) => StoreFault::NotFound(msg),
    }
}

impl ChangeStateStore for Store {
    fn get_change(&self, name: &str) -> Result<Option<ChangeStateRecord>, StoreFault> {
        self.find_change_record(name).map_err(fault)
    }

    fn list_change_records(&self) -> Result<Vec<ChangeStateRecord>, StoreFault> {
        self.list_change_records().map_err(fault)
    }

    fn list_phase_records(&self, change: &str) -> Result<Vec<PhaseStateRecord>, StoreFault> {
        Store::list_phase_records(self, change).map_err(fault)
    }

    fn list_steps(
        &self,
        change: &str,
        run_id: Option<&str>,
    ) -> Result<Vec<StepStateRecord>, StoreFault> {
        self.list_change_steps(change, run_id).map_err(fault)
    }

    fn create_change_record(&self, record: ChangeStateRecord) -> Result<(), StoreFault> {
        Store::create_change_record(self, record).map(|_| ()).map_err(fault)
    }

    fn delete_change_record(&self, name: &str) -> Result<bool, StoreFault> {
        Store::delete_change_record(self, name).map_err(fault)
    }

    fn start_phase(
        &self,
        change: &str,
        phase: &str,
        now: i64,
    ) -> Result<PhaseStartState, StoreFault> {
        self.start_change_phase(change, phase, now).map_err(fault)
    }

    fn log_phase(&self, command: &PhaseLogCommand) -> Result<u32, StoreFault> {
        self.log_change_phase(command).map_err(fault)
    }

    fn apply_backtrack(&self, command: &BacktrackCommand) -> Result<(), StoreFault> {
        self.apply_change_backtrack(command).map_err(fault)
    }

    fn amend_decision_session(
        &self,
        change: &str,
        phase: &str,
        session_id: &str,
    ) -> Result<(), StoreFault> {
        self.amend_change_decision_session(change, phase, session_id)
            .map_err(fault)
    }

    fn set_archived(&self, name: &str, archived_at: i64) -> Result<(), StoreFault> {
        self.set_change_archived(name, archived_at).map_err(fault)
    }

    fn append_step(&self, command: &StepCommand) -> Result<(), StoreFault> {
        self.append_change_step(command).map_err(fault)
    }
}

const _: () = {
    // port 契约 Send + Sync 硬校验（跨线程经 Arc<dyn ChangeStateStore> 注入）
    const fn assert_port<T: ChangeStateStore>() {}
    assert_port::<Store>();
};
