use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use exec::SystemCommand;
use foundation::cancel::CancelFlag;

use super::{mutation_jobs, SegmentFailure, PER_MUTANT_TIMEOUT_SECS, SEGMENT_SPAWN_TIMEOUT_MS};

/// cargo-mutants 段报告目录（spawn cwd 相对——默认写 `<cwd>/mutants.out`，
/// `--output` 未传）。
const CARGO_MUTANTS_OUT_DIR: &str = "mutants.out";

/// cargo-mutants 运行结果报告文件名（`mutants.out/outcomes.json`——逐突变体
/// outcome 与计数面的权威落位；运行面 stdout 只有人类可读进度行，`--json` 仅
/// 作用于 `--list`）。
const CARGO_OUTCOMES_FILE: &str = "outcomes.json";

/// 零突变体段合成载荷（计数面直读形态——`parse_cargo_mutants_seg` 计数面同式；
/// 全零计数 → 归并块 valid == 0 → score 100 恒过，空段不拖累 suite 判定）。
const ZERO_MUTANTS_PAYLOAD: &str = r#"{ "caught": 0, "missed": 0, "timeout": 0, "unviable": 0 }"#;

/// 突变段选用的 cargo profile 名（根 `Cargo.toml` 的 `[profile.mutants]`：
/// `inherits = "test"` + `debug = "none"`）。cargo 不会因定义了 profile 就自动
/// 采用，必须经 `--profile` 显式选中——实测不传时仍走默认 test profile
/// （`debuginfo=2`）。
const MUTANTS_CARGO_PROFILE: &str = "mutants";

/// `TAURI_CONFIG` 覆盖值：把 Tauri 配置里**相对路径**的 `version`（本项目为
/// `"../package.json"`）解析成字面版本号。解析不出 → `None`（**MUST NOT** 注入
/// 空串：`tauri_build` 对空 `TAURI_CONFIG` 按 JSON 解析并 panic `EOF while
/// parsing a value`，比不注入更糟）。
///
/// 根因：cargo-mutants 把源码树复制到临时目录再构建，复制根是 `src-tauri/`，而
/// `tauri.conf.json` 的 version 指向 `../package.json`（仓库里位于 `src-tauri/`
/// 的上级）。副本里那个上级文件不存在 → `tauri_build` 直接 panic
/// （`` `tauri.conf.json > version` must be a semver string ``），整个 `dev-team`
/// 包的每个突变体都被记 unviable（分母被排除 → 该包 score 恒 100 假绿）。
///
/// 该覆盖只影响 `tauri_build` 的版本读取（版本号本身不参与任何构建逻辑），
/// 副本仍在临时目录里构建，不触碰源树。
fn tauri_config_override(exec_dir: &Path) -> Option<String> {
    let version = read_tauri_version(exec_dir)?;
    Some(serde_json::json!({ "version": version }).to_string())
}

/// 读 `tauri.conf.json` 的 `version`，相对路径形态回退到**文件系统解析**：
/// `exec_dir/tauri.conf.json` 的 `version` 若形如相对路径，则拼同级目录再读其中
/// 的 `version` 字段（本项目即 `../package.json` → `src-tauri/../package.json`）。
/// 任何一步失败 → `None`（不覆盖，行为与未加本修复一致）。
fn read_tauri_version(exec_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(exec_dir.join("tauri.conf.json")).ok()?;
    let config: serde_json::Value = serde_json::from_str(&text).ok()?;
    let raw = config.get("version")?.as_str()?;
    // 已是字面 semver → 不动（覆盖无意义且可能遮掉配置问题）
    if !raw.contains('/') && !raw.contains('\\') {
        return Some(raw.to_owned());
    }
    let resolved = exec_dir.join(raw);
    let referenced = std::fs::read_to_string(&resolved).ok()?;
    let value: serde_json::Value = serde_json::from_str(&referenced).ok()?;
    value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// 段执行
pub(crate) async fn run_segment(
    patch_path: &Path,
    exec_dir: &Path,
    cancel: CancelFlag,
) -> Result<String, SegmentFailure> {
    // 目标目录用**相对路径** `target`：cargo 按各子进程自己的 cwd 解析，而
    // cargo-mutants 的每个 worker 各持一份独立临时副本——于是每个 worker 天然
    // 各写各的 target，互不共享。
    //
    // 共享同一绝对 `CARGO_TARGET_DIR` 是错的：rustc 的 `-C metadata` 由包名 /
    // 版本 / 编译参数算出，**不含源码路径与内容**，各副本落在同一 target 里就是
    // 同一组 fingerprint 与产物名。实测表现为 `Fresh` 复用**别的副本**编译出的
    // 二进制（源码已变却不重编 → 假 caught / 假 missed）与并发链接争用
    // （`LNK1104: cannot open file …deps/<crate>-<hash>.exe` → 假 unviable）。
    //
    // 用相对值而非「不注入」是为了盖掉父进程可能已全局导出的绝对
    // `CARGO_TARGET_DIR`（`env` 是覆盖语义，无需额外的清除字段）。
    // 跨段复用由段级复用门（零 spawn）承担，不靠共享 target dir。
    let env: Vec<(OsString, OsString)> =
        vec![(OsString::from("CARGO_TARGET_DIR"), OsString::from("target"))]
            .into_iter()
            .chain(
                tauri_config_override(exec_dir)
                    .map(|config| (OsString::from("TAURI_CONFIG"), OsString::from(config))),
            )
            .collect();

    // 段报告落位清场：上一段 / 上次运行的陈旧 outcomes.json MUST NOT 被本段误读
    // （best-effort——目录缺席等失败不阻断，spawn 后报告在位即权威）
    let _ = std::fs::remove_dir_all(exec_dir.join(CARGO_MUTANTS_OUT_DIR));
    let command = SystemCommand::bare("cargo")
        .args(["mutants", "--test-tool=nextest", "--workspace", "--in-diff"])
        .arg(patch_path)
        .args(["--baseline", "skip", "--jobs"])
        .arg(mutation_jobs().to_string())
        .args(["--timeout"])
        .arg(PER_MUTANT_TIMEOUT_SECS.to_string())
        // 只关调试符号的专用 profile（见根 Cargo.toml `[profile.mutants]`）：
        // 突变体预期「编译通过、测试失败」，符号价值远不及编译速度。
        .args(["--profile", MUTANTS_CARGO_PROFILE]);
    let run = exec::run_shell(
        command,
        exec::RunOptions {
            cwd: Some(exec_dir.to_path_buf()),
            env,
            timeout: Duration::from_millis(SEGMENT_SPAWN_TIMEOUT_MS),
            cancel,
            // 突变面零重试（长跑是产品语义，段内重试会放大墙钟；断点续跑已由段级
            // 复用门承担）
            retry: exec::RetryPolicy::none(),
            // 突变段零 live 接入（段内不产输出记录面，记录面扩张留待后续变更）
            live: None,
        },
    )
    .await;
    let (exit_code, output) = match run {
        Err(exec::RunError::Spawn(error)) => {
            return Err(SegmentFailure::Fatal(format!(
                "cargo mutants 拉起失败（程序不可达？）: {error}；请确认 cargo-mutants 已安装（cargo install cargo-mutants）"
            )));
        }
        Err(exec::RunError::Wait(error)) => {
            return Err(SegmentFailure::Fatal(format!(
                "cargo-mutants 段执行等待失败: {error}"
            )));
        }
        // 超时：整树已杀 + 收尸完成——段中断即时反馈面（双时点②）
        Ok(report) if report.outcome == exec::RunOutcome::TimedOut => {
            return Err(SegmentFailure::Interrupted(format!(
                "cargo-mutants 段执行超时（{} 秒）强制终止",
                SEGMENT_SPAWN_TIMEOUT_MS / 1000
            )));
        }
        // 取消：段中断面（MUST NOT 走 Fatal——不把用户停止呈现为环境损坏）
        Ok(report) if report.outcome == exec::RunOutcome::Cancelled => {
            return Err(SegmentFailure::Cancelled(
                "cargo-mutants 段执行已取消".to_owned(),
            ));
        }
        Ok(report) => {
            let mut output = report.stdout;
            output.extend_from_slice(&report.stderr);
            (report.exit_code, output)
        }
    };

    // 段报告为权威：`outcomes.json` 在位且可解析 → 段完成（cargo-mutants 以非零
    // 退出表达未达标等语义不中断）；退出 0 且无报告 → 零突变体段（合成空计数
    // 面）；其余（崩溃 / 无报告非零退出）→ 中断即时反馈
    let outcomes_path = exec_dir
        .join(CARGO_MUTANTS_OUT_DIR)
        .join(CARGO_OUTCOMES_FILE);
    match std::fs::read_to_string(&outcomes_path) {
        Ok(raw) if serde_json::from_str::<serde_json::Value>(&raw).is_ok() => Ok(raw),
        _ if exit_code == 0 => Ok(ZERO_MUTANTS_PAYLOAD.to_owned()),
        _ => Err(SegmentFailure::Interrupted(segment_failure_message(
            exit_code, &output,
        ))),
    }
}

/// 段中断错误面（非零退出 / stdout 无 JSON 载荷；错误信息 + 输出末尾双段
/// ——exec `ProcessOutput` 统一装配）。
fn segment_failure_message(exit_code: i32, output: &[u8]) -> String {
    let head = format!("cargo-mutants 段执行失败（退出码 {exit_code}，未产出可解析 JSON 段报告）");
    exec::ProcessOutput::new(exit_code, output.to_vec()).failure_message(&head)
}

#[cfg(test)]
#[path = "cargo_mutants_test.rs"]
mod cargo_mutants_test;
