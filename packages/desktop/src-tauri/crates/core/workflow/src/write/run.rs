use crate::state::{ChangeStateStore, RunFinishCommand, RunStartCommand, RunStatus};

/// run 发起落库（walker 起点第一写）：校验（run_id 非空 + change 建档在案）
/// 后经 [`ChangeStateStore::run_start`] 落 running 行。run_id 空白显式
/// `Err`（身份段缺失无从串链步史）；change 未建档（`get_change` `None`，文
/// 档形态存量 CLI change）显式 `Err`——run 编排不可达，运行史更无从落。
pub fn run_start(store: &dyn ChangeStateStore, command: &RunStartCommand) -> Result<(), String> {
    if command.run_id.trim().is_empty() {
        return Err("run_id 不得为空白（无从串链运行史）".to_owned());
    }
    let recorded = store
        .get_change(&command.change)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            format!(
                "change \"{}\" 未建档（无 ChangeRecord），无从落 run 史",
                command.change
            )
        })?;
    let _ = recorded;
    store.run_start(command).map_err(|error| error.to_string())
}

/// run 收口落库（walker 终态出口第二写）：校验（change 建档在案 + 命令
/// status 为终态三值且非 interrupted）后经 [`ChangeStateStore::run_finish`]
/// 单事务落（终态 + 步整包 + active_phase 清位）。`interrupted` 显式拒绝
/// ——该值仅启动标定产生，运行期写路径不产生（design D12 / 标定记因定式）。
pub fn run_finish(store: &dyn ChangeStateStore, command: &RunFinishCommand) -> Result<(), String> {
    let recorded = store
        .get_change(&command.change)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            format!(
                "change \"{}\" 未建档（无 ChangeRecord），无从落 run 终态",
                command.change
            )
        })?;
    let _ = recorded;
    if matches!(command.status, RunStatus::Running | RunStatus::Interrupted) {
        return Err(format!(
            "run 终态非法: \"{}\"（运行期写路径仅产生 completed / stopped / failed；\
             interrupted 仅启动标定写入）",
            command.status.as_str()
        ));
    }
    store.run_finish(command).map_err(|error| error.to_string())
}
