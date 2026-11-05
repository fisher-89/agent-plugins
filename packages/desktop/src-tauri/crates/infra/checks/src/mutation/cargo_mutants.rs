use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use exec::SystemCommand;
use foundation::cancel::CancelFlag;

use super::{
    mutation_jobs, segments, SegmentFailure, MUTATION_TEST_THREADS, PER_MUTANT_TIMEOUT_SECS,
};

/// cargo-mutants 段报告目录（spawn cwd 相对——默认写 `<cwd>/mutants.out`，
/// `--output` 未传）。
const CARGO_MUTANTS_OUT_DIR: &str = "mutants.out";

/// cargo-mutants 的旧报告轮换目录名：起跑时若发现 `mutants.out` 已在位，工具会
/// 把整个旧目录改名到此（而不是删除），逐次运行无限堆积。段执行 spawn 前连同
/// `mutants.out` 一并清场（见 `run_segment`）。
const CARGO_MUTANTS_OUT_OLD_DIR: &str = "mutants.out.old";

/// cargo-mutants 运行结果报告文件名（`mutants.out/outcomes.json`——逐突变体
/// outcome 与计数面的权威落位；运行面 stdout 只有人类可读进度行，`--json` 仅
/// 作用于 `--list`）。
const CARGO_OUTCOMES_FILE: &str = "outcomes.json";

/// 突变 scratch 目录名（相对 suite 执行目录的 `target/`——见 `run_suite`
/// 的 TEMP / TMP / TMPDIR 重定向注释）。
const MUTANTS_SCRATCH_DIR: &str = "mutants-scratch";

/// 增量成果文件（`mutants.out/` 下）：cargo-mutants **边跑边写**每行的判定结果，
/// 而 `outcomes.json` 只在正常退出时落盘。执行被强杀时这四个文件是部分成果的
/// 唯一来源；文件名即 outcome 词（与计数面同词）。
const CARGO_PARTIAL_FILES: [(&str, &str); 4] = [
    ("caught.txt", "caught"),
    ("missed.txt", "missed"),
    ("timeout.txt", "timeout"),
    ("unviable.txt", "unviable"),
];

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

/// 整体执行（不切段）
///
/// `mutant_count` = 本次突变的**估算**突变体数（`--list` 精确值不可得时的兜底
/// 密度 × 文件数），仅用于折算 spawn 硬超时；spawn 处 MUST NOT 行内算。
///
/// 返回 `(原始报告, 是否为中断回读)`：`true` 表示执行被超时 / 取消终止，报告由
/// [`recover_partial`] 从增量成果文件合成——已判完的突变体不丢。
pub(crate) async fn run_suite(
    patch_path: &Path,
    exec_dir: &Path,
    mutant_count: u64,
    cancel: CancelFlag,
) -> Result<(String, bool), SegmentFailure> {
    let spawn_timeout_ms = segments::run_spawn_timeout_ms(mutant_count);
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
    let mut env: Vec<(OsString, OsString)> =
        vec![(OsString::from("CARGO_TARGET_DIR"), OsString::from("target"))];
    if let Some(config) = tauri_config_override(exec_dir) {
        env.push((OsString::from("TAURI_CONFIG"), OsString::from(config)));
    }
    // worker 副本（scratch）落位：cargo-mutants 默认把源码树复制到系统临时目录
    // （`std::env::temp_dir()` → TEMP / TMP / TMPDIR）。实测系统盘（C:）临时区
    // 的单进程启动成本 = 同源盘（D:）的 2~3 倍（0.9 s vs 0.45 s），在 nextest
    // 逐用例起进程的放大下整包测试从 29 s 涨到 78 s。故把三个键一并重定向到
    // `<exec_dir>/target/mutants-scratch`：
    // - 与源码同盘（消除跨盘复制与系统盘实时扫描开销）；
    // - 落在 `target/` 内——顶层 `target` 已被 cargo-mutants 排除复制，放别处会
    //   把 scratch 自身复制进每个副本；`target` 名亦被复用门扫描剪枝。
    // 建目录失败即不注入（静默回退到系统临时目录，行为与未加本改动一致）。
    let scratch = exec_dir.join("target").join(MUTANTS_SCRATCH_DIR);
    if std::fs::create_dir_all(&scratch).is_ok() {
        let scratch = scratch.into_os_string();
        for key in ["TEMP", "TMP", "TMPDIR"] {
            env.push((OsString::from(key), scratch.clone()));
        }
    }

    // 段报告落位清场：上一段 / 上次运行的陈旧 outcomes.json MUST NOT 被本段误读
    // （best-effort——目录缺席等失败不阻断，spawn 后报告在位即权威）
    let _ = std::fs::remove_dir_all(exec_dir.join(CARGO_MUTANTS_OUT_DIR));
    // cargo-mutants 起跑时若发现 `mutants.out` 已在位，会把**整个旧目录**改名为
    // `mutants.out.old`（无限堆积、且是 `.gitignore` 只覆盖 `mutants.out` 时的
    // 漏网目录）。上一行的清场已消掉这条路径；这里再兜底清一次历史残留
    // （早前手工运行留下的），best-effort。
    let _ = std::fs::remove_dir_all(exec_dir.join(CARGO_MUTANTS_OUT_OLD_DIR));
    let command = SystemCommand::bare("cargo")
        .args(["mutants", "--test-tool=nextest", "--workspace", "--in-diff"])
        .arg(patch_path)
        .args(["--baseline", "skip", "--jobs"])
        .arg(mutation_jobs().to_string())
        .args(["--timeout"])
        .arg(PER_MUTANT_TIMEOUT_SECS.to_string())
        // 只关调试符号的专用 profile（见根 Cargo.toml `[profile.mutants]`）：
        // 突变体预期「编译通过、测试失败」，符号价值远不及编译速度。
        .args(["--profile", MUTANTS_CARGO_PROFILE])
        // worker 副本按 `.gitignore` 剪枝（cargo-mutants 默认**不看** gitignore，
        // 会把未跟踪的构建产物一并复制）。本仓库 `crates/target`（llvm-cov 的
        // 构建目录，~225MB）虽已从版本库撤出，但仍在工作区里——不剪枝时每份副本
        // 是 246MB / 962 文件，剪枝后 9MB / 493 文件（实测）。
        //
        // 安全性：被剪掉的只是未跟踪 / 已忽略的文件；构建所需的一切都在跟踪集内
        // （`gen/` 由 tauri-build 在副本内自行重建，`crates/target` 本来就不参与
        // 构建）。根包已实测可正常编译（含 tauri_build）。
        .args(["--gitignore", "true"])
        // 每 worker 内的测试线程数：cargo-mutants 只把 `--cargo-test-arg` 追加到
        // Test 阶段（不进 `--check` 的 `cargo check`），nextest 档下落在
        // `cargo nextest run` 之后，正是它自己的 `-j` 位。不设则默认吃满核，
        // `jobs × 满核` 会把机器压到测试互相饿死（实测纯函数突变体撞满超时）。
        .args(["--cargo-test-arg"])
        .arg(format!("-j{MUTATION_TEST_THREADS}"));
    let run = exec::run_shell(
        command,
        exec::RunOptions {
            cwd: Some(exec_dir.to_path_buf()),
            env,
            timeout: Duration::from_millis(spawn_timeout_ms),
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
        // 超时：整树已杀 + 收尸完成。**先尝试回读部分成果**——增量文件在盘即
        // 已判完的突变体不丢；一件都读不到才走中断面（双时点②）。
        Ok(report) if report.outcome == exec::RunOutcome::TimedOut => {
            return recover_or(exec_dir, || {
                SegmentFailure::Interrupted(format!(
                    "cargo-mutants 执行超时（{} 秒）强制终止",
                    spawn_timeout_ms / 1000
                ))
            });
        }
        // 取消：同上——已判完的成果保留，回读不到才走取消面（MUST NOT 走 Fatal）
        Ok(report) if report.outcome == exec::RunOutcome::Cancelled => {
            return recover_or(exec_dir, || {
                SegmentFailure::Cancelled("cargo-mutants 执行已取消".to_owned())
            });
        }
        Ok(report) => {
            let mut output = report.stdout;
            output.extend_from_slice(&report.stderr);
            (report.exit_code, output)
        }
    };

    // 报告为权威：`outcomes.json` 在位且可解析 → 完整执行（cargo-mutants 以非零
    // 退出表达未达标等语义不中断）；退出 0 且无报告 → 零突变体（合成空计数面）；
    // 其余（崩溃 / 无报告非零退出）→ 先试回读部分成果，再走中断面
    let outcomes_path = exec_dir
        .join(CARGO_MUTANTS_OUT_DIR)
        .join(CARGO_OUTCOMES_FILE);
    match std::fs::read_to_string(&outcomes_path) {
        Ok(raw) if serde_json::from_str::<serde_json::Value>(&raw).is_ok() => Ok((raw, false)),
        _ if exit_code == 0 => Ok((ZERO_MUTANTS_PAYLOAD.to_owned(), false)),
        _ => recover_or(exec_dir, || {
            SegmentFailure::Interrupted(segment_failure_message(exit_code, &output))
        }),
    }
}

/// 中断面回读：从 `mutants.out/` 的增量成果文件合成计数面。
///
/// cargo-mutants 边跑边往 `caught.txt` / `missed.txt` / `timeout.txt` /
/// `unviable.txt` 追加一行一突变体，而 `outcomes.json` 只在正常退出时落盘。故超时 /
/// 取消时这四个文件是部分成果的唯一来源。
///
/// 返回 `(合成载荷, true)`；一个文件都读不到（或全为空）时调 `fallback` 产出原
/// 失败面——**不把「零成果」伪装成「跑完了」**。
fn recover_or(
    exec_dir: &Path,
    fallback: impl FnOnce() -> SegmentFailure,
) -> Result<(String, bool), SegmentFailure> {
    match recover_partial(exec_dir) {
        Some(raw) => Ok((raw, true)),
        None => Err(fallback()),
    }
}

/// 增量成果文件 → 计数面载荷（`{"caught": N, "missed": N, …}`）。
///
/// 行数即计数（每行一个突变体名，含坐标与替换描述）。零总行数时返回 `None`——
/// 那是「一件都没判完」，与「全被 caught」语义相反，MUST NOT 混同。
fn recover_partial(exec_dir: &Path) -> Option<String> {
    let mut counts = serde_json::Map::new();
    let mut total = 0u64;
    for (file, key) in CARGO_PARTIAL_FILES {
        let Ok(text) = std::fs::read_to_string(exec_dir.join(CARGO_MUTANTS_OUT_DIR).join(file))
        else {
            continue;
        };
        let count = text.lines().filter(|line| !line.trim().is_empty()).count() as u64;
        if count > 0 {
            total = total.saturating_add(count);
            counts.insert(key.to_owned(), serde_json::Value::from(count));
        }
    }
    if total == 0 {
        return None;
    }
    counts.insert("total_mutants".to_owned(), serde_json::Value::from(total));
    serde_json::to_string(&serde_json::Value::Object(counts)).ok()
}

/// 执行失败面（非零退出 / 无可用报告；错误信息 + 输出末尾双段——exec
/// `ProcessOutput` 统一装配）。
fn segment_failure_message(exit_code: i32, output: &[u8]) -> String {
    let head = format!("cargo-mutants 执行失败（退出码 {exit_code}，未产出可解析 JSON 报告）");
    exec::ProcessOutput::new(exit_code, output.to_vec()).failure_message(&head)
}

#[cfg(test)]
#[path = "cargo_mutants_test.rs"]
mod cargo_mutants_test;
