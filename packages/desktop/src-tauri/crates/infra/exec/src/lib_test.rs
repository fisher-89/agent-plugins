//! SystemCommand 组装形状测试：经 `into_std` 的 get_program / get_args
//! 断言壳包装与 argv 形态（无进程 spawn；`creation_flags` 进程级不可观测，
//! 由 release 手工验收覆盖）。

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::{ShellBase, SystemCommand};

#[test]
fn script按平台默认壳组装() {
    let command = SystemCommand::script("echo hi").into_std();
    #[cfg(windows)]
    {
        assert_eq!(command.get_program(), OsStr::new("cmd"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new("/C"), OsStr::new("echo hi")]
        );
    }
    #[cfg(not(windows))]
    {
        assert_eq!(command.get_program(), OsStr::new("sh"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new("-c"), OsStr::new("echo hi")]
        );
    }
}

#[test]
fn script_raw形态与script同程序且末参原位() {
    // raw 标记仅 tokio 侧（raw_arg）表达，std 侧 argv 形状与 script 一致
    let command = SystemCommand::script_raw("echo hi").into_std();
    #[cfg(windows)]
    assert_eq!(command.get_program(), OsStr::new("cmd"));
    #[cfg(not(windows))]
    assert_eq!(command.get_program(), OsStr::new("sh"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            OsStr::new(if cfg!(windows) { "/C" } else { "-c" }),
            OsStr::new("echo hi")
        ]
    );
}

#[test]
fn script_with按底座组装() {
    let bash = PathBuf::from("C:\\Git\\bin\\bash.exe");
    let gitbash =
        SystemCommand::script_with(&ShellBase::GitBash(bash.clone()), "echo hi").into_std();
    assert_eq!(gitbash.get_program(), OsStr::new(&bash));
    assert_eq!(
        gitbash.get_args().collect::<Vec<_>>(),
        [OsStr::new("-c"), OsStr::new("echo hi")]
    );

    let cmd = SystemCommand::script_with(&ShellBase::Cmd, "echo hi").into_std();
    assert_eq!(cmd.get_program(), OsStr::new("cmd"));
    assert_eq!(
        cmd.get_args().collect::<Vec<_>>(),
        [OsStr::new("/C"), OsStr::new("echo hi")]
    );

    #[cfg(not(windows))]
    {
        let sh = SystemCommand::script_with(&ShellBase::Sh, "echo hi").into_std();
        assert_eq!(sh.get_program(), OsStr::new("sh"));
        assert_eq!(
            sh.get_args().collect::<Vec<_>>(),
            [OsStr::new("-c"), OsStr::new("echo hi")]
        );
    }
}

#[test]
fn bare逐参直传零壳包装() {
    let command = SystemCommand::bare("git")
        .arg("-C")
        .arg("/root")
        .args(["status", "--porcelain"])
        .into_std();
    assert_eq!(command.get_program(), OsStr::new("git"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            OsStr::new("-C"),
            OsStr::new("/root"),
            OsStr::new("status"),
            OsStr::new("--porcelain")
        ]
    );
}

// ---------------------------------------------------------------------------
// ExecOutput 落盘 / 回查（test-design「infra/exec/src/lib.rs -> lib_test.rs」节：
// 输出记录单点收敛——单一交错 / 合并流落单一 `<exec_id>.log`，`.err` 分文件
// 退役；AC-6）
// ---------------------------------------------------------------------------

use foundation::layout::ReportSlot;

use crate::{read_exec_output, read_exec_output_in, record_exec_output};

/// 临时 workspace 根 RAII（报告目录树落盘 + 回查真实往返）。
struct TempRoot(std::path::PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("exec-lib-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时根失败");
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }

    /// 输出记录目录（`<report_dir>/exec-outputs`——落盘与回查同锚推导）。
    fn exec_dir(&self, change: &str) -> std::path::PathBuf {
        foundation::layout::change_test_reports(self.0.as_path(), change).join("exec-outputs")
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 正向：单一输出流落盘 → 仅 `<exec_id>.log`（`.err` 退役零残留），返回非空
/// `exec_id`；回查逐字节一致（引用式零改写零截断）。
#[test]
fn record_exec_output落盘并回查逐字节一致() {
    let root = TempRoot::new("roundtrip");
    // `2>&1` 交错流（static-check）与 stdout + stderr 合并流（test-execution）
    // 同型：单一字节流保真，分离快照不再存在
    let output = b"clippy: warning: unused import\nerror[E0308]: mismatched types\n".to_vec();

    let exec_id = record_exec_output(root.path(), "demo-change", ReportSlot::Test, &output).expect("落盘应成功");
    assert!(!exec_id.is_empty(), "exec_id 非空");
    assert!(
        exec_id.starts_with("exec-"),
        "铸造定式 exec-<millis>-<序号>: {exec_id}"
    );

    let read_back = read_exec_output(root.path(), "demo-change", &exec_id).expect("回查应命中");
    assert_eq!(read_back, output, "回查全量字节与写入一致");

    let dir = root.exec_dir("demo-change");
    assert!(
        dir.join(format!("{exec_id}.log")).is_file(),
        "落盘位于报告目录树 exec-outputs/<exec_id>.log"
    );
    assert!(
        !dir.join(format!("{exec_id}.err")).exists(),
        "stderr 分文件退役（单一 .log 承载全量）"
    );
    let entries: Vec<String> = std::fs::read_dir(&dir)
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        entries,
        [format!("{exec_id}.log")],
        "单次落盘恰一个文件（无 .err 残留）"
    );
}

/// 正向：两次落盘 exec_id 互异（引用唯一面）；change 名隔离（不同 change 目录
/// 各自回查）。
#[test]
fn record_exec_output两次铸造互异且change隔离() {
    let root = TempRoot::new("unique");
    let first = record_exec_output(root.path(), "change-a", ReportSlot::Test, b"one").expect("落盘应成功");
    let second = record_exec_output(root.path(), "change-a", ReportSlot::Test, b"two").expect("落盘应成功");
    assert_ne!(first, second, "两次铸造 exec_id 互异");

    let third = record_exec_output(root.path(), "change-b", ReportSlot::Test, b"three").expect("落盘应成功");
    assert_eq!(
        read_exec_output(root.path(), "change-a", &first).expect("回查应命中"),
        b"one".to_vec()
    );
    assert_eq!(
        read_exec_output(root.path(), "change-b", &third).expect("回查应命中"),
        b"three".to_vec()
    );
    // 跨 change 回查同一 exec_id → None（目录隔离）
    assert!(read_exec_output(root.path(), "change-b", &first).is_none());
}

/// 正向：`read_exec_output` 读同一 `.log`——已知 `exec_id` → `Some(内容)`，与
/// 写入逐字一致（与 `record_exec_output` 同口径）。
#[test]
fn read_exec_output已知引用回读同一log() {
    let root = TempRoot::new("same-log");
    let output = "首个 suite 输出\n第二个 suite 输出\n".as_bytes().to_vec();
    let exec_id = record_exec_output(root.path(), "demo", ReportSlot::Test, &output).expect("落盘应成功");

    let read_back = read_exec_output(root.path(), "demo", &exec_id).expect("回查应命中");
    assert_eq!(read_back, output);
    // 重复回读恒同字节（读面无副作用）
    assert_eq!(
        read_exec_output(root.path(), "demo", &exec_id).expect("重复回查应命中"),
        output
    );
}

/// 正向：`read_exec_output_in`（change 目录定位变体）——目录由 `locate_change`
/// 类三态定位面供给，任意锚（worktree / active / archive）record 的 `.log` 经
/// change 目录回读逐字节一致；与 root 变体同构（后缀单点）。
#[test]
fn read_exec_output_in_change目录定位与root变体同构() {
    // 异锚落盘：模拟 worktree 锚（与 root 非同根）
    let anchor = TempRoot::new("in-anchor");
    let output = "worktree suite 输出\n".as_bytes().to_vec();
    let exec_id = record_exec_output(anchor.path(), "demo", ReportSlot::Test, &output).expect("落盘应成功");
    let change_dir = foundation::layout::resolve(anchor.path())
        .changes_root
        .join("demo");

    assert_eq!(
        read_exec_output_in(&change_dir, &exec_id).expect("change 目录定位应命中"),
        output,
        "change 目录变体回读与写入逐字节一致"
    );
    // 同构：root 变体等值（reports/test 后缀单点派生）
    assert_eq!(
        read_exec_output_in(&change_dir, &exec_id),
        read_exec_output(anchor.path(), "demo", &exec_id),
        "两变体同锚等值"
    );
    // 校验语义同 root 变体：未知 / 非法 exec_id → None（miss 非错误）
    assert!(read_exec_output_in(&change_dir, "exec-unknown").is_none());
    for hostile in ["", "..", "a/b", "exec-1.log"] {
        assert!(
            read_exec_output_in(&change_dir, hostile).is_none(),
            "非法 exec_id {hostile:?} 应拒绝（零路径逃逸）"
        );
    }
}

/// 异常：未知 exec_id / 非法形态（空串 / `..` / 路径分隔符 / 盘符 / `.log`
/// 后缀）→ None（miss 非错误，且零路径逃逸）。
#[test]
fn read_exec_output未知与非法exec_id返回none() {
    let root = TempRoot::new("miss");
    let exec_id = record_exec_output(root.path(), "demo", ReportSlot::Test, b"payload").expect("落盘应成功");

    assert!(read_exec_output(root.path(), "demo", "exec-unknown").is_none());
    for hostile in ["", "..", "../secret", "a/b", r"a\b", "C:evil", "exec-1.log"] {
        assert!(
            read_exec_output(root.path(), "demo", hostile).is_none(),
            "非法 exec_id {hostile:?} 应拒绝"
        );
    }
    assert!(read_exec_output(root.path(), "demo", &exec_id).is_some());
    // 未知 change 名 → 目录不存在 → None（miss 非错误）
    assert!(read_exec_output(root.path(), "ghost-change", &exec_id).is_none());
}

/// 边界：单一输出流全空 → 落盘空内容仍可回查（`Some(vec![])`）；超长 + 特殊
/// 字节（`\n` / `\0` / emoji）全量保真（引用式不截断）。
#[test]
fn record_exec_output空与超长特殊字节保真() {
    let root = TempRoot::new("bytes");
    let empty = record_exec_output(root.path(), "demo", ReportSlot::Test, b"").expect("空输出落盘应成功");
    assert_eq!(
        read_exec_output(root.path(), "demo", &empty).expect("空输出应可回查"),
        Vec::<u8>::new(),
        "全空 → Some(vec![])"
    );

    let mut long = Vec::new();
    for index in 0..2000 {
        long.extend_from_slice(format!("行 {index} 🚀\n").as_bytes());
    }
    long.push(0);
    let exec_id = record_exec_output(root.path(), "demo", ReportSlot::Test, &long).expect("超长落盘应成功");
    let read_back = read_exec_output(root.path(), "demo", &exec_id).expect("回查应命中");
    assert_eq!(read_back.len(), long.len(), "超长全量保真（不截断）");
    assert_eq!(read_back, long, "含 \0 与 emoji 字节逐字保真");
}

/// 异常：落盘目录不可建（报告树路径被文件占位）→ `Err(String)` 串语义（不静
/// 默吞失败）。
#[test]
fn record_exec_output目录不可写时err串语义() {
    let root = TempRoot::new("unwritable");
    let reports = foundation::layout::change_test_reports(root.path(), "demo");
    std::fs::create_dir_all(reports.parent().expect("父目录")).expect("创建父目录失败");
    std::fs::write(&reports, "占位文件").expect("占位写失败");

    let err = record_exec_output(root.path(), "demo", ReportSlot::Test, b"stdout").expect_err("应 Err");
    assert!(
        err.contains("exec") || err.contains("目录") || err.contains("落盘"),
        "Err 串语境保留: {err}"
    );
}

/// 幂等：重复 `run` 形态的多次落盘 → 每次新铸互异 `exec_id`、各写独立 `.log`
/// 互不覆写（前次记录不被改写 / 不被清理），重复回读同 `exec_id` 恒同字节；
/// 全程零 `.err` 残留（无副作用）。
#[test]
fn record_exec_output重复落盘互不覆写且零err残留() {
    let root = TempRoot::new("idempotent");
    let mut ids: Vec<String> = Vec::new();
    let mut contents: Vec<Vec<u8>> = Vec::new();
    for index in 0..3 {
        let output = format!("第 {index} 次执行输出\n").into_bytes();
        let exec_id = record_exec_output(root.path(), "demo", ReportSlot::Test, &output).expect("落盘应成功");
        ids.push(exec_id);
        contents.push(output);
    }

    assert_eq!(
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "三次落盘 exec_id 互异"
    );
    for (exec_id, output) in ids.iter().zip(contents.iter()) {
        assert_eq!(
            read_exec_output(root.path(), "demo", exec_id).expect("各自回查应命中"),
            output.clone(),
            "前次记录不被覆写（各 exec_id 各自保真）"
        );
    }
    // 重复回读同引用恒同字节（读面不迁移不清理）
    assert_eq!(
        read_exec_output(root.path(), "demo", &ids[0]).expect("重复回查应命中"),
        contents[0]
    );

    let mut names: Vec<String> = std::fs::read_dir(root.exec_dir("demo"))
        .expect("读输出记录目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut expected: Vec<String> = ids.iter().map(|id| format!("{id}.log")).collect();
    expected.sort();
    assert_eq!(names, expected, "仅 .log 落盘（零 .err 残留、零覆写）");
}

// ---------------------------------------------------------------------------
// slot 槽位与双锚回查（check-before-flow-start / AC-1 / AC-8）：输出记录按槽位
// 锚定 + `read_exec_output_in` 双锚探测（Mock策略：无——真实 tempdir 报告树落
// 盘 + 回查往返，fs 为被测组合面）
// ---------------------------------------------------------------------------

/// record slot=Baseline 正向：输出落 `<change>/reports/baseline/exec-outputs/
/// <exec_id>.log`（单一 `.log` 零 `.err` 残留）；exec_id 非空且重复调用互异
///（铸造零改动锚）；test 树零混居。
#[test]
fn record_exec_output_slot_baseline落baseline树且零err残留() {
    let root = TempRoot::new("baseline-record");
    let first = record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"one").expect("落盘应成功");
    let second = record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"two").expect("落盘应成功");
    assert_ne!(first, second, "重复铸造互异");

    let baseline_dir =
        foundation::layout::change_reports(root.path(), "demo", ReportSlot::Baseline)
            .join("exec-outputs");
    let mut names: Vec<String> = std::fs::read_dir(&baseline_dir)
        .expect("读 baseline 输出目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut expected: Vec<String> = [first.clone(), second.clone()]
        .iter()
        .map(|id| format!("{id}.log"))
        .collect();
    expected.sort();
    assert_eq!(names, expected, "恰两份 .log（零 .err 残留）: {names:?}");

    // test 树零混居
    assert!(
        !root.exec_dir("demo").exists(),
        "test 树零混居（slot=Baseline 不落 reports/test）"
    );
}

/// read_exec_output_in 双锚（正向）：baseline 树 exec_id → 首锚 test miss →
/// 次锚 baseline 命中（内容逐字节）；test 树 exec_id → 首锚命中即返；同一
/// change 两树并存输出各自回查可达。
#[test]
fn read_exec_output_in双锚_baseline与test两树各自可达() {
    let root = TempRoot::new("dual-anchor");
    let baseline_id =
        record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"baseline output").expect("落盘应成功");
    let test_id = record_exec_output(root.path(), "demo", ReportSlot::Test, b"test output").expect("落盘应成功");
    let change_dir = foundation::layout::resolve(root.path()).changes_root.join("demo");

    // baseline 树 id：首锚 miss → 次锚命中
    assert_eq!(
        read_exec_output_in(&change_dir, &baseline_id).expect("baseline id 应命中"),
        b"baseline output".to_vec(),
        "次锚 baseline 命中且内容逐字节"
    );
    // test 树 id：首锚命中即返
    assert_eq!(
        read_exec_output_in(&change_dir, &test_id).expect("test id 应命中"),
        b"test output".to_vec(),
        "首锚 test 命中且内容逐字节"
    );
}

/// read_exec_output_in 跨树唯一（边界）：两树各落一份输出 → 各自 id 回查得各
/// 自内容、无串树（exec_id 跨树唯一——首中即返不歧义）；两树均 miss 的未知 id
/// → None。
#[test]
fn read_exec_output_in跨树唯一无串树() {
    let root = TempRoot::new("cross-tree");
    let baseline_id =
        record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"from-baseline-tree").expect("落盘应成功");
    let test_id = record_exec_output(root.path(), "demo", ReportSlot::Test, b"from-test-tree").expect("落盘应成功");
    let change_dir = foundation::layout::resolve(root.path()).changes_root.join("demo");

    // 各自 id 回查各自内容（零串树）
    assert_ne!(
        read_exec_output_in(&change_dir, &baseline_id).expect("baseline 命中"),
        read_exec_output_in(&change_dir, &test_id).expect("test 命中"),
        "两树内容可辨（零串树）"
    );
    assert_eq!(
        read_exec_output_in(&change_dir, &baseline_id).expect("baseline 命中"),
        b"from-baseline-tree".to_vec()
    );
    // 两树均 miss 的未知 id → None
    assert!(read_exec_output_in(&change_dir, "exec-999-none").is_none());
}

/// read_exec_output 顶层组合（组合，AC-1 / AC-8）：顶层函数签名零改动——经
/// root 变体回查两树 id 均可达（委托 `read_in` 天然双锚——IPC `read_exec_output`
/// 命令零改动回查覆盖面的组合半边）。
#[test]
fn read_exec_output顶层组合_经root变体两树id均可达() {
    let root = TempRoot::new("top-combo");
    let baseline_id =
        record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"top baseline").expect("落盘应成功");
    let test_id = record_exec_output(root.path(), "demo", ReportSlot::Test, b"top test").expect("落盘应成功");

    assert_eq!(
        read_exec_output(root.path(), "demo", &baseline_id).expect("顶层回查 baseline 应命中"),
        b"top baseline".to_vec(),
        "root 变体双锚覆盖 baseline 树"
    );
    assert_eq!(
        read_exec_output(root.path(), "demo", &test_id).expect("顶层回查 test 应命中"),
        b"top test".to_vec(),
        "root 变体双锚覆盖 test 树"
    );
}

/// exec_output_dir 槽位版对拍（正向，枚举遍历）：slot 两值目录与
/// `foundation::layout::change_reports_in(change_dir, slot)` 等值（锚点单点委托
/// ——`exec_output_dir_in` 无第二份后缀拼接）。
#[test]
fn exec_output_dir槽位版与layout单点等值() {
    let root = TempRoot::new("dir-derive");
    let change_dir = foundation::layout::resolve(root.path()).changes_root.join("demo");
    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        assert_eq!(
            super::exec_output_dir(root.path(), "demo", slot),
            foundation::layout::change_reports_in(&change_dir, slot).join("exec-outputs"),
            "slot={slot:?} 目录派生与 layout 单点等值（exec-outputs 后缀单点）"
        );
        assert_eq!(
            super::exec_output_dir_in(&change_dir, slot),
            super::exec_output_dir(root.path(), "demo", slot),
            "slot={slot:?} 两变体同构（仅前缀差异）"
        );
    }
}


// ---------------------------------------------------------------------------
// SystemCommand clone 等价（test-design「lib.rs -> lib_test.rs」节：`#[derive(Debug)]
// → #[derive(Debug, Clone)]`——`run_shell` 消费命令规格前需可复用）
// ---------------------------------------------------------------------------

/// 正向：壳包装 clone 后两份各自 `into_std()`，program / argv 逐字相等；原值
/// clone 后仍可继续使用（未 move）。
#[test]
fn system_command_clone等价_壳包装() {
    let command = SystemCommand::script("echo hi");
    let cloned = command.clone();

    let original_std = command.into_std();
    let cloned_std = cloned.into_std();
    assert_eq!(
        original_std.get_program(),
        cloned_std.get_program(),
        "clone 后 program 逐字相等"
    );
    assert_eq!(
        original_std.get_args().collect::<Vec<_>>(),
        cloned_std.get_args().collect::<Vec<_>>(),
        "clone 后 argv 逐字相等"
    );
    assert_eq!(
        original_std.get_program(),
        OsStr::new(if cfg!(windows) { "cmd" } else { "sh" }),
        "原值 clone 后仍可继续使用（未 move）"
    );
}

/// 正向（表驱动 4 形态）：`bare` / `script` / `script_raw` / `script_with`
/// 逐形态 clone → program + argv 逐字相等。
#[test]
fn system_command_clone等价_四形态() {
    let bash = PathBuf::from("C:/Git/bin/bash.exe");
    let cases: Vec<(&str, SystemCommand)> = vec![
        (
            "bare 带 argv",
            SystemCommand::bare("cargo").args(["mutants", "--workspace"]),
        ),
        ("script", SystemCommand::script("echo hi")),
        ("script_raw", SystemCommand::script_raw("npx stryker run")),
        (
            "script_with(GitBash)",
            SystemCommand::script_with(&ShellBase::GitBash(bash), "echo hi"),
        ),
    ];

    for (label, command) in cases {
        let cloned = command.clone();
        let original_std = command.into_std();
        let cloned_std = cloned.into_std();
        assert_eq!(
            original_std.get_program(),
            cloned_std.get_program(),
            "{label}: program 逐字相等"
        );
        assert_eq!(
            original_std.get_args().collect::<Vec<_>>(),
            cloned_std.get_args().collect::<Vec<_>>(),
            "{label}: argv 逐字相等"
        );
    }
}

/// 边界：零 argv——`bare("cargo")` clone 后 program 相等、`get_args()` 为空
/// （不 panic）。
#[test]
fn system_command_clone等价_零argv() {
    let cloned = SystemCommand::bare("cargo").clone();
    let command = cloned.into_std();
    assert_eq!(command.get_program(), OsStr::new("cargo"));
    assert_eq!(
        command.get_args().count(),
        0,
        "零参数 clone 后仍为空 argv（不 panic）"
    );
}

/// 边界（参数化 4 形态）：特殊字符与超长 argv / 非 UTF-8 `OsString`——clone 后
/// 逐字节相等（`OsStr` 的非 UTF-8 面保留）。
#[test]
fn system_command_clone等价_特殊字符与超长与非utf8() {
    let long = "z".repeat(1200);
    #[cfg(unix)]
    let non_utf8 = {
        use std::os::unix::ffi::OsStringExt as _;
        std::ffi::OsString::from_vec(vec![b'b', 0xFF, 0xFE])
    };
    #[cfg(not(unix))]
    let non_utf8 = std::ffi::OsString::from("非UTF8占位\u{0}");

    let args: Vec<std::ffi::OsString> = vec![
        std::ffi::OsString::from("he said \"hi\""),
        std::ffi::OsString::from("line-1\nline-2"),
        std::ffi::OsString::from("emoji 🎉🚀"),
        std::ffi::OsString::from(long),
        non_utf8,
    ];
    let command = SystemCommand::bare("prog").args(args.clone());
    let cloned = command.clone();

    let original_std = command.into_std();
    let cloned_std = cloned.into_std();
    assert_eq!(
        original_std.get_args().collect::<Vec<_>>(),
        args.iter().map(std::ffi::OsString::as_os_str).collect::<Vec<_>>(),
        "原始 argv 逐字"
    );
    assert_eq!(
        cloned_std.get_args().collect::<Vec<_>>(),
        args.iter().map(std::ffi::OsString::as_os_str).collect::<Vec<_>>(),
        "clone 后 argv 逐字节相等（含非 UTF-8 / 超长 / 特殊字符）"
    );
}

/// 边界：`raw` 标记保真——`script_raw` clone 后末参仍为命令串（`raw == true`
/// 语义不因 clone 退化为 false；Windows 侧唯一可观测面在 tokio `raw_arg`，此处
/// 以 `into_tokio().as_std()` 对拍 program + 末参）。
#[test]
fn system_command_clone等价_raw标记保真() {
    let script = "npx --prefix 'C:/suite' stryker run '{config}' > '{x}'";
    let cloned = SystemCommand::script_raw(script).clone();
    let tokio_command = cloned.into_tokio();
    let std_command = tokio_command.as_std();

    assert_eq!(
        std_command.get_program(),
        OsStr::new(if cfg!(windows) { "cmd" } else { "sh" }),
        "raw 形态不变（clone 后仍为平台壳）"
    );
    let args: Vec<_> = std_command.get_args().collect();
    assert_eq!(args.len(), 2, "壳标记 + 末参恰两参: {args:?}");
    assert_eq!(args[1], OsStr::new(script), "末参命令串逐字保真");
}
