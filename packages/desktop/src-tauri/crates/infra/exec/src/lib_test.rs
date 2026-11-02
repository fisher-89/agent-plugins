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

    let exec_id = record_exec_output(root.path(), "demo-change", ReportSlot::Test, &output)
        .expect("落盘应成功");
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
    let first =
        record_exec_output(root.path(), "change-a", ReportSlot::Test, b"one").expect("落盘应成功");
    let second =
        record_exec_output(root.path(), "change-a", ReportSlot::Test, b"two").expect("落盘应成功");
    assert_ne!(first, second, "两次铸造 exec_id 互异");

    let third = record_exec_output(root.path(), "change-b", ReportSlot::Test, b"three")
        .expect("落盘应成功");
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
    let exec_id =
        record_exec_output(root.path(), "demo", ReportSlot::Test, &output).expect("落盘应成功");

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
    let exec_id =
        record_exec_output(anchor.path(), "demo", ReportSlot::Test, &output).expect("落盘应成功");
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
    let exec_id =
        record_exec_output(root.path(), "demo", ReportSlot::Test, b"payload").expect("落盘应成功");

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
    let empty =
        record_exec_output(root.path(), "demo", ReportSlot::Test, b"").expect("空输出落盘应成功");
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
    let exec_id =
        record_exec_output(root.path(), "demo", ReportSlot::Test, &long).expect("超长落盘应成功");
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

    let err =
        record_exec_output(root.path(), "demo", ReportSlot::Test, b"stdout").expect_err("应 Err");
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
        let exec_id =
            record_exec_output(root.path(), "demo", ReportSlot::Test, &output).expect("落盘应成功");
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
    let first =
        record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"one").expect("落盘应成功");
    let second =
        record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"two").expect("落盘应成功");
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
    let baseline_id = record_exec_output(
        root.path(),
        "demo",
        ReportSlot::Baseline,
        b"baseline output",
    )
    .expect("落盘应成功");
    let test_id = record_exec_output(root.path(), "demo", ReportSlot::Test, b"test output")
        .expect("落盘应成功");
    let change_dir = foundation::layout::resolve(root.path())
        .changes_root
        .join("demo");

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
    let baseline_id = record_exec_output(
        root.path(),
        "demo",
        ReportSlot::Baseline,
        b"from-baseline-tree",
    )
    .expect("落盘应成功");
    let test_id = record_exec_output(root.path(), "demo", ReportSlot::Test, b"from-test-tree")
        .expect("落盘应成功");
    let change_dir = foundation::layout::resolve(root.path())
        .changes_root
        .join("demo");

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
        record_exec_output(root.path(), "demo", ReportSlot::Baseline, b"top baseline")
            .expect("落盘应成功");
    let test_id =
        record_exec_output(root.path(), "demo", ReportSlot::Test, b"top test").expect("落盘应成功");

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
    let change_dir = foundation::layout::resolve(root.path())
        .changes_root
        .join("demo");
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
        args.iter()
            .map(std::ffi::OsString::as_os_str)
            .collect::<Vec<_>>(),
        "原始 argv 逐字"
    );
    assert_eq!(
        cloned_std.get_args().collect::<Vec<_>>(),
        args.iter()
            .map(std::ffi::OsString::as_os_str)
            .collect::<Vec<_>>(),
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

// ---------------------------------------------------------------------------
// 再导出面（`mod transient;` / `mod lockprobe;` 的 crate 根可见性）
// ---------------------------------------------------------------------------

/// 正向：再导出 `is_transient`——经 `crate::is_transient`（而非模块私有路径）
/// 判定同一组输入，与瞬态模块自身判据逐值一致（导出的是同一函数，非包装）。
#[test]
fn 再导出is_transient与模块判据一致() {
    let red_locked = crate::RunReport {
        outcome: crate::RunOutcome::Completed,
        exit_code: 1,
        stdout: b"error: failed to rename archive file: /t/liba.rlib: os error 32".to_vec(),
        stderr: Vec::new(),
        attempts: 2,
        retried: Vec::new(),
    };
    assert_eq!(
        crate::is_transient(&red_locked),
        Some("os error 32"),
        "crate 根导出的判定函数命中签名"
    );

    let green = crate::RunReport {
        outcome: crate::RunOutcome::Completed,
        exit_code: 0,
        stdout: b"os error 32".to_vec(),
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    };
    assert_eq!(crate::is_transient(&green), None, "绿不因文本命中");
}

/// 正向（表驱动 2 类型）：再导出 `RetryPolicy` / `AttemptTrace`——经 crate 根
/// 构造两策略与留痕，三常量与类型表等值。
#[test]
fn 再导出retry_policy与attempt_trace() {
    let none = crate::RetryPolicy::none();
    let transient = crate::RetryPolicy::transient();
    assert_eq!(none.max_attempts, 1, "单次策略上限 1");
    assert_eq!(transient.max_attempts, 5, "瞬态策略上限 5");
    assert_eq!(
        transient.backoff,
        [
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(20),
            std::time::Duration::from_secs(40),
        ],
        "退避四段（慢档 5 / 10 / 20 / 40 s）"
    );
    assert_eq!(
        transient.total_budget,
        std::time::Duration::from_secs(1500),
        "总预算 1500 s"
    );

    let trace = crate::AttemptTrace {
        exit_code: 1,
        transient: Some("EBUSY".to_owned()),
        stdout: b"out".to_vec(),
        stderr: b"err".to_vec(),
    };
    assert_eq!(trace.exit_code, 1, "留痕字段可读");
    assert_eq!(trace.transient.as_deref(), Some("EBUSY"));
    assert_eq!(trace.clone(), trace, "clone / PartialEq 可用");
}

/// 正向（表驱动 4 项）：再导出 `locked_paths` / `lock_holders` /
/// `attribution_text` / `LockHolder`——纯函数与类型经 crate 根可用且行为与模块
/// 内一致。
#[test]
fn 再导出锁探针四项() {
    assert_eq!(
        crate::locked_paths("error: failed to write /tmp/x.rmeta"),
        vec![PathBuf::from("/tmp/x.rmeta")],
        "crate 根导出的路径解析"
    );
    assert!(
        crate::lock_holders(&[]).is_empty(),
        "空入参点名直接返回空（零系统调用）"
    );
    assert_eq!(
        crate::attribution_text("error[E0308]: mismatched types"),
        None,
        "无路径可解析 → None"
    );

    let holder = crate::LockHolder {
        name: "HipsDaemon.exe".to_owned(),
        pid: 4321,
    };
    assert_eq!(
        holder.clone(),
        crate::LockHolder {
            name: "HipsDaemon.exe".to_owned(),
            pid: 4321,
        },
        "LockHolder 可在 crate 根构造并比较"
    );
}

/// 边界：既有导出面零变——`kill_tree(None)` 零副作用、`ProcessOutput`
/// 错误面 / 末尾面口径逐字不变（被 `environment::fault_text` 复用）。
#[test]
fn 既有导出面零变() {
    // `kill_tree(None)`：无 pid 即直接返回（零 spawn、零 panic）
    crate::kill_tree(None);

    let output =
        crate::ProcessOutput::new(101, b"line-1\nwarning: x\nerror[E0308]: boom\n".to_vec());
    let excerpt = output.error_excerpt();
    assert_eq!(
        excerpt, "error[E0308]: boom",
        "错误面口径不变（含 error 的行）"
    );
    assert!(
        output.tail().contains("error[E0308]: boom"),
        "末尾面不剥行（行口径 50 行）"
    );
    assert_eq!(
        crate::ProcessOutput::new(1, Vec::new()).tail(),
        "",
        "空输出末尾面为空"
    );
}

// ---------------------------------------------------------------------------
// 再导出面（D15：增量推进判据三函数的 crate 根可见性）
// ---------------------------------------------------------------------------

/// 正向（表驱动 3 项，D15）：`compiling_count` / `progress_series` /
/// `is_progress_stalled` 经 **crate 根**可达，且与模块内判据逐值一致（导出的是
/// 同一函数，非包装）。
#[test]
fn 再导出推进判据三函数() {
    // `compiling_count`
    assert_eq!(
        crate::compiling_count(b"   Compiling a v0.1.0\n", b""),
        1,
        "crate 根计数函数命中 rustc 缩进形态"
    );
    assert_eq!(
        crate::compiling_count(b"   Compiling a v0.1.0\n", b""),
        crate::transient::compiling_count(b"   Compiling a v0.1.0\n", b""),
        "根导出与模块内逐值一致"
    );

    // `progress_series`
    let report = crate::RunReport {
        outcome: crate::RunOutcome::Completed,
        exit_code: 1,
        stdout: b"   Compiling a v0.1.0\n   Compiling b v0.1.0\n".to_vec(),
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    };
    assert_eq!(
        crate::progress_series(&report),
        vec![2],
        "crate 根序列函数按尝试序取数"
    );
    assert_eq!(
        crate::progress_series(&report),
        crate::transient::progress_series(&report),
        "根导出与模块内逐值一致"
    );

    // `is_progress_stalled`
    assert!(
        crate::is_progress_stalled(&[50, 50]),
        "crate 根卡死判据命中末两次相等"
    );
    assert_eq!(
        crate::is_progress_stalled(&[50, 50]),
        crate::transient::is_progress_stalled(&[50, 50]),
        "根导出与模块内逐值一致"
    );
}

/// 异常（表驱动 3 形态）：退化输入经 crate 根取值与模块内直调逐值一致（空序列 /
/// 双空字节 / 全 0 序列 → `false` / `0` / `false`），不 panic。
#[test]
fn 再导出面边界输入一致性() {
    assert!(!crate::is_progress_stalled(&[]), "空序列不 panic");
    assert_eq!(
        crate::is_progress_stalled(&[]),
        crate::transient::is_progress_stalled(&[]),
        "空序列根导出与模块内一致"
    );

    assert_eq!(crate::compiling_count(b"", b""), 0, "双空字节计数 0");
    assert_eq!(
        crate::compiling_count(b"", b""),
        crate::transient::compiling_count(b"", b""),
        "双空字节根导出与模块内一致"
    );

    assert!(!crate::is_progress_stalled(&[0, 0]), "全 0 序列判否");
    assert_eq!(
        crate::is_progress_stalled(&[0, 0]),
        crate::transient::is_progress_stalled(&[0, 0]),
        "全 0 序列根导出与模块内一致"
    );
}

/// 边界（编译期面）：`checks` 侧 `use exec::{progress_series, is_progress_stalled};`
/// 的取数路径在用例内以 `crate::` 路径解析成功表达——任一新项未再导出即编译失败
/// （本函数即解析点）。
#[test]
fn 导出面路径解析成功() {
    let series = crate::progress_series(&crate::RunReport {
        outcome: crate::RunOutcome::Completed,
        exit_code: 1,
        stdout: Vec::new(),
        stderr: Vec::new(),
        attempts: 1,
        retried: Vec::new(),
    });
    assert_eq!(series.len(), 1, "一次尝试即长度 1");
    assert!(
        !crate::is_progress_stalled(&series),
        "单值序列不判卡死（检查域视图取数面）"
    );
}

// ---------------------------------------------------------------------------
// 运行中记录与增量回查（change-detail-live-stdout，AC-1 / AC-2 / AC-3 / AC-4）
//
// 被测面：`LiveExec`（begin / exec_id / log_path / sink / write_command /
// commit）、`record_exec_output` 薄包装等价、`read_exec_command_in` /
// `exec_output_log_path_in` / `read_exec_output_delta_in` 三只读函数，以及
// `LiveSink` / `ExecOutputDelta` 两类型形态。
//
// Mock 策略（见 test-design 本节 Mock策略表）：零替身——真实 tempdir 报告树
//（`TempRoot` RAII，fs 即被测组合面）、真实 `foundation::layout` 槽位派生、
// crate 内直构 `LiveSink::new` / `LiveExec`（制造公开 API 不可触达的失败臂，
// 不是模块替身）；零 spawn、零时钟注入（`mint_exec_id` 只做形态断言）。
// ---------------------------------------------------------------------------

use std::sync::{Arc, Mutex};

use crate::{
    exec_output_log_path_in, is_valid_exec_id, read_exec_command_in, read_exec_output_delta_in,
    ExecOutputDelta, LiveExec, LiveSink,
};

/// change 目录（与 `exec_output_dir` 同锚的 change 目录变体入参）。
fn change_dir_of(root: &std::path::Path, change: &str) -> std::path::PathBuf {
    foundation::layout::resolve(root).changes_root.join(change)
}

/// 槽位输出记录目录（`reports/<slot>/exec-outputs`）。
fn slot_dir(root: &std::path::Path, change: &str, slot: ReportSlot) -> std::path::PathBuf {
    foundation::layout::change_reports(root, change, slot).join("exec-outputs")
}

/// 目录条目名集合（升序；零 fs 副作用断言的取数面）。
fn entry_names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("读目录")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// 正向：`begin` 铸造 `exec-<millis>-<seq>` 形态的 `exec_id`，父目录与 0 字节
/// `.log` 在返回时已在案（AC-1 的前置：watch 订阅可生效的前提）。
#[test]
fn live_exec_begin建空log且父目录先于exec_id可见() {
    let root = TempRoot::new("live-begin");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin 应成功");

    let exec_id = live.exec_id().to_owned();
    assert!(!exec_id.is_empty(), "exec_id 非空");
    let segments: Vec<&str> = exec_id.split('-').collect();
    assert_eq!(segments.len(), 3, "定式 exec-<millis>-<seq>: {exec_id}");
    assert_eq!(segments[0], "exec", "前缀定式: {exec_id}");
    assert!(
        segments[1].chars().all(|ch| ch.is_ascii_digit()),
        "millis 段为数字: {exec_id}"
    );
    assert!(
        segments[2].chars().all(|ch| ch.is_ascii_digit()),
        "seq 段为数字: {exec_id}"
    );

    let dir = slot_dir(root.path(), "demo", ReportSlot::Test);
    assert!(dir.is_dir(), "父目录 exec-outputs/ 已创建");
    let log = dir.join(format!("{exec_id}.log"));
    assert!(log.is_file(), "空 .log 已建");
    assert_eq!(
        std::fs::metadata(&log).expect("读 .log 元数据").len(),
        0,
        ".log 起步 0 字节"
    );
    assert_eq!(live.log_path(), log.as_path(), "log_path 与布局派生同值");
}

/// 正向：`slot = Baseline` 落 `reports/baseline/exec-outputs/`，test 树零新增
///（槽位同锚派生）。
#[test]
fn live_exec_begin_baseline槽位落baseline树且test树零新增() {
    let root = TempRoot::new("live-baseline");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Baseline).expect("begin 应成功");
    let exec_id = live.exec_id().to_owned();

    let baseline_dir = slot_dir(root.path(), "demo", ReportSlot::Baseline);
    assert!(
        baseline_dir.join(format!("{exec_id}.log")).is_file(),
        "baseline 树落 .log"
    );
    assert!(
        !root.exec_dir("demo").exists(),
        "test 树零新增（槽位不混居）"
    );
}

/// 异常：报告树路径被普通文件占位 → `Err(String)` 带语境、不 panic、不留半成品
/// `.log`（记录面失败是显式 `Err`，由调用方静默降级）。
#[test]
fn live_exec_begin目录被占位时err串带语境且不留半成品() {
    let root = TempRoot::new("live-begin-err");
    let reports = foundation::layout::change_test_reports(root.path(), "demo");
    std::fs::create_dir_all(reports.parent().expect("父目录")).expect("建父目录");
    std::fs::write(&reports, "占位文件").expect("占位写失败");

    // `LiveExec` 零 `Debug`（不出线、不派生的有意收窄），故以 match 取 Err 臂
    let err = match LiveExec::begin(root.path(), "demo", ReportSlot::Test) {
        Ok(_) => panic!("报告树被文件占位时应 Err"),
        Err(error) => error,
    };
    assert!(
        err.contains("目录") || err.contains("失败"),
        "Err 串带创建语境: {err}"
    );
    assert!(!root.exec_dir("demo").exists(), "失败不留半成品输出目录");
}

/// 边界（参数化 4 形态）：change 名普通 / 超长（>200 chars）/ 含 `-` 与中文三
/// 形态正常铸造且目录落位；空串按 layout 派生（只断言不 panic 且返回
/// `Result`，不断言业务语义）。
#[test]
fn live_exec_begin_change名四形态不panic() {
    let root = TempRoot::new("live-begin-names");
    let long = "c".repeat(220);
    for change in ["plain-change", long.as_str(), "-含中文与-短横"] {
        let live = LiveExec::begin(root.path(), change, ReportSlot::Test)
            .unwrap_or_else(|error| panic!("change={change:?} 应成功: {error}"));
        let exec_id = live.exec_id().to_owned();
        assert!(
            slot_dir(root.path(), change, ReportSlot::Test)
                .join(format!("{exec_id}.log"))
                .is_file(),
            "change={change:?} 目录落位"
        );
    }

    // 空串：layout 派生（changes_root 自身）——只锁「不 panic 且返回 Result」
    match LiveExec::begin(root.path(), "", ReportSlot::Test) {
        Ok(live) => assert!(!live.exec_id().is_empty(), "空 change 仍铸造 id"),
        Err(error) => assert!(!error.is_empty(), "失败亦带语境串: {error}"),
    }
}

/// 正向 / 异常：`commit` 前后 `exec_id()` 恒同值、`log_path()` 与
/// `exec_output_log_path_in` 同值且绝对；两次 `begin` 的 id 与路径互异。
#[test]
fn live_exec_exec_id与log_path_commit前后同值且两次互异() {
    let root = TempRoot::new("live-id");
    let first = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let first_id = first.exec_id().to_owned();
    let change_dir = change_dir_of(root.path(), "demo");

    assert_eq!(
        exec_output_log_path_in(&change_dir, &first_id).as_deref(),
        Some(first.log_path()),
        "log_path 与双锚探测同值"
    );
    assert!(first.log_path().is_absolute(), "log_path 为绝对路径");
    assert!(
        first
            .log_path()
            .to_string_lossy()
            .ends_with(&format!("{first_id}.log")),
        "末尾为 <exec_id>.log"
    );

    let committed = first.commit(b"authoritative\n").expect("commit 应成功");
    assert_eq!(committed, first_id, "commit 返回同一 id（同 id 收口）");
    assert_eq!(first.exec_id(), first_id, "commit 后 exec_id 不变");

    let second = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    assert_ne!(second.exec_id(), first_id, "两次铸造互异");
    assert_ne!(second.log_path(), first.log_path(), "两次落点互异");
    assert_eq!(
        exec_output_log_path_in(&change_dir, second.exec_id()).as_deref(),
        Some(second.log_path())
    );
}

/// 边界：`log_path()` 反复调用返回同一值、零 fs 副作用（父目录条目集合不变）。
#[test]
fn live_exec_log_path反复读取零fs副作用() {
    let root = TempRoot::new("live-log-path");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let dir = slot_dir(root.path(), "demo", ReportSlot::Test);
    let before = entry_names(&dir);

    let first = live.log_path().to_path_buf();
    let second = live.log_path().to_path_buf();
    let third = live.log_path().to_path_buf();
    assert_eq!(first, second);
    assert_eq!(second, third);
    assert_eq!(entry_names(&dir), before, "反复读取零 fs 副作用");
}

/// 正向：`sink()` 追加可见、顺序保序——两段 `absorb` 后以增量读回逐字内容，
/// `next_offset` 等于累计字节数。
#[test]
fn live_exec_sink追加可见且顺序保序() {
    let root = TempRoot::new("live-sink-order");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let sink = live.sink();
    sink.absorb(b"line-1\n");
    sink.absorb(b"line-2\n");

    let delta = read_exec_output_delta_in(&change_dir_of(root.path(), "demo"), live.exec_id(), 0)
        .expect("运行中应可读");
    assert_eq!(delta.text, "line-1\nline-2\n", "追加可见且保序");
    assert_eq!(delta.next_offset, 14, "游标 == 累计字节数");
}

/// 正向：同一 `LiveExec` 取两个 sink（模拟两路 drain 并发）→ 交替各 100 次
/// 投递，文件字节数 == 全部片段长度之和（`Mutex` 支撑并发写、零丢失）。
#[test]
fn live_exec_sink两路并发投递零丢失() {
    let root = TempRoot::new("live-sink-two");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let stdout_sink = live.sink();
    let stderr_sink = live.sink();

    let mut expected = 0usize;
    for index in 0..100 {
        let out = format!("out-{index}\n");
        let err = format!("err-{index}\n");
        expected += out.len() + err.len();
        stdout_sink.absorb(out.as_bytes());
        stderr_sink.absorb(err.as_bytes());
    }

    let bytes = std::fs::read(live.log_path()).expect("读 live 文件");
    assert_eq!(bytes.len(), expected, "全部片段落盘、零丢失");
    let text = String::from_utf8(bytes).expect("全 ASCII 文本");
    assert!(text.contains("out-0\n"), "首段在场");
    assert!(text.contains("err-99\n"), "末段在场");
}

/// 异常：写失败静默——`writer` 置为只读句柄 → `absorb` 不 panic 不上抛、句柄
/// 置 `None`（此后恒空操作），`commit` 仍覆盖为权威内容（观测面不阻断主流程）。
#[test]
fn live_exec_sink写失败静默且commit仍覆盖() {
    let root = TempRoot::new("live-sink-fail");
    let dir = slot_dir(root.path(), "demo", ReportSlot::Test);
    std::fs::create_dir_all(&dir).expect("建目录");
    let log_path = dir.join("exec-1-0.log");
    std::fs::write(&log_path, b"").expect("建空文件");
    let read_only = std::fs::OpenOptions::new()
        .read(true)
        .open(&log_path)
        .expect("以只读句柄打开");

    let live = LiveExec {
        exec_id: "exec-1-0".to_owned(),
        log_path: log_path.clone(),
        writer: Arc::new(Mutex::new(Some(read_only))),
    };
    let sink = live.sink();
    sink.absorb(b"should-not-land");
    sink.absorb(b"nor-this");
    assert_eq!(
        std::fs::read(&log_path).expect("读文件"),
        Vec::<u8>::new(),
        "写失败零落盘、零 panic、零上抛"
    );

    let committed = live.commit(b"authoritative\n").expect("commit 仍成功");
    assert_eq!(committed, "exec-1-0");
    assert_eq!(
        std::fs::read(&log_path).expect("读文件"),
        b"authoritative\n".to_vec(),
        "句柄失效后 commit 仍以权威内容覆盖"
    );
}

/// 边界（参数化 4 形态）：`absorb(&[])` 长度不变；8 KiB 恰满块与其前后各 1
/// 字节三形态全部落盘（chunk 边界不丢字节）。
#[test]
fn live_exec_sink零字节与chunk边界字节全落盘() {
    for size in [0usize, 8191, 8192, 8193] {
        let root = TempRoot::new(&format!("live-sink-chunk-{size}"));
        let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
        let sink = live.sink();
        let payload = vec![b'a'; size];
        sink.absorb(&payload);
        assert_eq!(
            std::fs::metadata(live.log_path()).expect("stat").len() as usize,
            size,
            "size={size} 全部落盘（零丢失）"
        );
        if size == 0 {
            let delta =
                read_exec_output_delta_in(&change_dir_of(root.path(), "demo"), live.exec_id(), 0)
                    .expect("空文件亦命中");
            assert_eq!(delta.text, "");
            assert_eq!(delta.next_offset, 0);
        }
    }
}

/// 边界：`commit` 之后再 `absorb` → 文件内容仍等于权威内容（追加恒空操作，
/// 单一状态位 `writer = None` 承载，D1）。
#[test]
fn live_exec_sink_commit后追加恒空操作() {
    let root = TempRoot::new("live-sink-late");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let sink = live.sink();
    sink.absorb(b"interleaved");
    live.commit(b"authoritative\n").expect("commit");

    sink.absorb(b"late");
    assert_eq!(
        std::fs::read(live.log_path()).expect("读文件"),
        b"authoritative\n".to_vec(),
        "收口后追加恒空操作"
    );
}

/// 正向：`write_command` 原样追加——`read_exec_command_in` 逐字返回该串（无尾随
/// 换行、含 `2>&1`），文件位于 `.log` 同目录、名 `<exec_id>.cmd`（AC-3）。
#[test]
fn live_exec_write_command原样追加且同目录同id前缀() {
    let root = TempRoot::new("live-cmd");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let script = "pnpm run check 2>&1";
    live.write_command(script).expect("写 sidecar 应成功");

    let change_dir = change_dir_of(root.path(), "demo");
    assert_eq!(
        read_exec_command_in(&change_dir, live.exec_id()).as_deref(),
        Some(script),
        "逐字返回（零尾随换行零改写）"
    );
    let cmd_path =
        slot_dir(root.path(), "demo", ReportSlot::Test).join(format!("{}.cmd", live.exec_id()));
    assert!(cmd_path.is_file(), "sidecar 与 .log 同目录");
    assert_eq!(
        cmd_path.parent(),
        live.log_path().parent(),
        "同目录（同 id 前缀）"
    );
    assert_eq!(
        std::fs::read(&cmd_path).expect("读 sidecar"),
        script.as_bytes()
    );
}

/// 正向：连续两次 `write_command`（test-execution 两段形态）→ 回查为两段原样
/// 拼接（原样追加、无二次格式化）。
#[test]
fn live_exec_write_command两段原样拼接() {
    let root = TempRoot::new("live-cmd-two");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let first = "\n=== plan a · rust ===\ncmd-a\n";
    let second = "\n=== plan b · vp ===\ncmd-b\n";
    live.write_command(first).expect("第一段");
    live.write_command(second).expect("第二段");

    assert_eq!(
        read_exec_command_in(&change_dir_of(root.path(), "demo"), live.exec_id()),
        Some(format!("{first}{second}")),
        "两段原样拼接（零二次格式化）"
    );
}

/// 异常：`.cmd` 目标位置被同名目录占位 → `Err(String)`、`.log` 面零影响、
/// `commit` 仍成功（两路互不共用状态位）。
#[test]
fn live_exec_write_command失败时err且log面零影响() {
    let root = TempRoot::new("live-cmd-err");
    let dir = slot_dir(root.path(), "demo", ReportSlot::Test);
    std::fs::create_dir_all(&dir).expect("建目录");
    let exec_id = "exec-2-0";
    std::fs::create_dir_all(dir.join(format!("{exec_id}.cmd"))).expect("占位目录");
    let log_path = dir.join(format!("{exec_id}.log"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("建 .log");
    let live = LiveExec {
        exec_id: exec_id.to_owned(),
        log_path: log_path.clone(),
        writer: Arc::new(Mutex::new(Some(file))),
    };

    let err = live.write_command("cmd").expect_err("目录占位应 Err");
    assert!(
        err.contains("命令记录") || err.contains("失败"),
        "Err 串带语境: {err}"
    );
    live.sink().absorb(b"live-bytes");
    assert_eq!(
        std::fs::read(&log_path).expect("读 .log"),
        b"live-bytes".to_vec(),
        ".log 面零影响"
    );
    assert_eq!(
        live.commit(b"authoritative\n").expect("commit 仍成功"),
        exec_id,
        "sidecar 失败不阻断收口"
    );
}

/// 边界（参数化 4 形态）：空串 / 超长（>1000 chars）/ 含 `\n` 与 `\0` 与 emoji
/// / 含 `\r\n` → 全部逐字节原样写入与回查（MUST NOT 追加换行、MUST NOT 转义）。
#[test]
fn live_exec_write_command四形态逐字节保真() {
    let long = "x".repeat(1200);
    let cases: [(&str, String); 4] = [
        ("空串", String::new()),
        ("超长", long),
        ("特殊字符", "line-1\n\u{0}emoji 🚀".to_owned()),
        ("CRLF", "a\r\nb\r\n".to_owned()),
    ];
    for (label, command) in cases {
        let root = TempRoot::new(&format!("live-cmd-form-{}", label.len()));
        let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
        live.write_command(&command)
            .unwrap_or_else(|error| panic!("{label}: 应成功: {error}"));
        let cmd_path =
            slot_dir(root.path(), "demo", ReportSlot::Test).join(format!("{}.cmd", live.exec_id()));
        assert_eq!(
            std::fs::read(&cmd_path).expect("读 sidecar"),
            command.as_bytes(),
            "{label}: 逐字节原样（零追加零转义）"
        );
        assert_eq!(
            read_exec_command_in(&change_dir_of(root.path(), "demo"), live.exec_id()),
            Some(command),
            "{label}: 回查逐字一致"
        );
    }
}

/// 正向：运行中追加交错流后 `commit` → 返回 id 同 `begin`，`.log` 逐字节等于
/// 权威内容（live 交错流被整体覆盖，AC-2）。
#[test]
fn live_exec_commit同id覆盖live交错流() {
    let root = TempRoot::new("live-commit");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let exec_id = live.exec_id().to_owned();
    live.sink().absorb(b"interleaved");
    assert_eq!(
        std::fs::read(live.log_path()).expect("读 live"),
        b"interleaved".to_vec()
    );

    let committed = live.commit(b"authoritative\n").expect("commit");
    assert_eq!(committed, exec_id, "同 id 收口");
    assert_eq!(
        std::fs::read(live.log_path()).expect("读收口内容"),
        b"authoritative\n".to_vec(),
        "权威内容整体覆盖交错流"
    );
}

/// 异常：`log_path` 指向预建目录 → `commit` 返回 `Err(String)` 不 panic；再次
/// `commit` 仍 `Err`（句柄已 take 的幂等失败面）。
#[test]
fn live_exec_commit失败不panic且幂等失败() {
    let root = TempRoot::new("live-commit-err");
    let dir_path = root.path().join("log-as-dir");
    std::fs::create_dir_all(&dir_path).expect("建目录占位");
    let live = LiveExec {
        exec_id: "exec-3-0".to_owned(),
        log_path: dir_path,
        writer: Arc::new(Mutex::new(None)),
    };

    let first = live.commit(b"payload").expect_err("路径是目录应 Err");
    assert!(
        first.contains("落盘") || first.contains("失败"),
        "Err 串带落盘语境: {first}"
    );
    assert!(live.commit(b"payload").is_err(), "再次 commit 仍 Err");
}

/// 边界（参数化 3 形态）：`commit(&[])` 截断为 0 字节且返回 id；超长（>1 MiB）
/// 与非法 UTF-8 字节逐字节保真（零截断零改写）。
#[test]
fn live_exec_commit空超长与非法字节三形态保真() {
    // 空内容（test-execution 复用门的空记录形态）
    let empty_root = TempRoot::new("live-commit-empty");
    let empty_live = LiveExec::begin(empty_root.path(), "demo", ReportSlot::Test).expect("begin");
    empty_live.sink().absorb(b"pre-existing");
    let empty_id = empty_live.commit(b"").expect("空 commit 应成功");
    assert_eq!(empty_id, empty_live.exec_id(), "返回 id 在案");
    assert_eq!(
        std::fs::read(empty_live.log_path()).expect("读文件"),
        Vec::<u8>::new(),
        "截断为 0 字节"
    );

    // 超长（>1 MiB）
    let long_root = TempRoot::new("live-commit-long");
    let long_live = LiveExec::begin(long_root.path(), "demo", ReportSlot::Test).expect("begin");
    let mut long: Vec<u8> = Vec::new();
    for index in 0..300_000u32 {
        long.extend_from_slice(index.to_string().as_bytes());
    }
    assert!(long.len() > 1024 * 1024, "构造超 1 MiB 内容");
    long_live.commit(&long).expect("超长 commit");
    assert_eq!(
        std::fs::read(long_live.log_path()).expect("读文件"),
        long,
        "超长逐字节保真"
    );

    // 非法 UTF-8 字节
    let raw_root = TempRoot::new("live-commit-raw");
    let raw_live = LiveExec::begin(raw_root.path(), "demo", ReportSlot::Test).expect("begin");
    let raw = b"head\xff\xfe tail \x00 \xe4\xb8".to_vec();
    raw_live.commit(&raw).expect("非法字节 commit");
    assert_eq!(
        std::fs::read(raw_live.log_path()).expect("读文件"),
        raw,
        "非法字节逐字节保真（零改写零截断）"
    );
}

/// 边界（组合 2 API × 4 字节形态）：`record_exec_output` 降为
/// `LiveExec::begin` + `commit` 薄包装的等价性——落点目录名与文件形态一致
/// （仅 `exec_id` 值不同）、内容逐字节相等。
#[test]
fn record_exec_output与liveexec_begin_commit薄包装等价() {
    let cases: [(&str, Vec<u8>); 4] = [
        ("空", Vec::new()),
        ("超长", vec![b'z'; 4096]),
        ("非法字节", b"a\xff\xfeb".to_vec()),
        ("emoji", "行一 🚀\n行二 📦\n".as_bytes().to_vec()),
    ];
    for (label, bytes) in cases {
        let root = TempRoot::new(&format!("thin-wrapper-{}", label.len()));
        let wrapped = record_exec_output(root.path(), "demo", ReportSlot::Test, &bytes)
            .unwrap_or_else(|error| panic!("{label}: 薄包装应成功: {error}"));
        let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
        let committed = live.commit(&bytes).expect("begin+commit");

        assert_ne!(wrapped, committed, "{label}: 两次铸造 id 互异");
        let dir = slot_dir(root.path(), "demo", ReportSlot::Test);
        assert!(
            dir.join(format!("{wrapped}.log")).is_file(),
            "{label}: 薄包装落点"
        );
        assert!(
            dir.join(format!("{committed}.log")).is_file(),
            "{label}: 直构落点"
        );
        assert_eq!(
            read_exec_output(root.path(), "demo", &wrapped).expect("薄包装回查"),
            bytes,
            "{label}: 薄包装内容逐字节"
        );
        assert_eq!(
            read_exec_output(root.path(), "demo", &committed).expect("直构回查"),
            bytes,
            "{label}: 直构内容逐字节"
        );
    }
}

/// 正向（参数化 2 槽）：`read_exec_command_in` 双锚各自可达——Test 槽命中 /
/// Baseline 槽命中。
#[test]
fn read_exec_command_in双锚各自可达() {
    let root = TempRoot::new("cmd-dual");
    let change_dir = change_dir_of(root.path(), "demo");
    for slot in [ReportSlot::Test, ReportSlot::Baseline] {
        let live = LiveExec::begin(root.path(), "demo", slot).expect("begin");
        let command = format!("cmd-for-{slot:?} 2>&1");
        live.write_command(&command).expect("写 sidecar");
        assert_eq!(
            read_exec_command_in(&change_dir, live.exec_id()),
            Some(command),
            "slot={slot:?} 槽位 sidecar 命中"
        );
    }
}

/// 异常：老记录（只有 `.log`、无 `.cmd`）→ `None`（非错误）；两槽皆无 → `None`。
#[test]
fn read_exec_command_in老记录与两槽皆无返回none() {
    let root = TempRoot::new("cmd-none");
    let change_dir = change_dir_of(root.path(), "demo");
    let exec_id =
        record_exec_output(root.path(), "demo", ReportSlot::Test, b"only-log").expect("落盘");
    assert!(
        read_exec_command_in(&change_dir, &exec_id).is_none(),
        "老记录无 sidecar → None"
    );
    assert!(
        read_exec_command_in(&change_dir, "exec-404-0").is_none(),
        "两槽皆无 → None"
    );
}

/// 边界（参数化 7 形态）：非法 `exec_id` 全部 `None`（路径逃逸零命中）。
#[test]
fn read_exec_command_in非法exec_id七形态全部none() {
    let root = TempRoot::new("cmd-hostile");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    live.write_command("cmd").expect("写 sidecar");
    // 真实记录在场，证明 None 出自 id 校验而非文件缺失
    assert!(read_exec_command_in(&change_dir, live.exec_id()).is_some());

    for hostile in ["", "..", "../secret", "a/b", r"a\b", "C:evil", "exec-1.cmd"] {
        assert!(
            read_exec_command_in(&change_dir, hostile).is_none(),
            "非法 exec_id {hostile:?} 应拒绝（零路径逃逸）"
        );
        assert!(!is_valid_exec_id(hostile), "{hostile:?} 形态校验应否决");
    }
}

/// 正向：`exec_output_log_path_in`——`begin` 后返回绝对路径且 `is_file()`、与
/// `LiveExec::log_path()` 等值、`commit` 后仍命中。
#[test]
fn exec_output_log_path_in命中且与live_log_path等值() {
    let root = TempRoot::new("log-path-hit");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");

    let path = exec_output_log_path_in(&change_dir, live.exec_id()).expect("begin 后应命中");
    assert!(path.is_absolute(), "绝对路径");
    assert!(path.is_file(), "命中即文件在场");
    assert_eq!(path, live.log_path(), "与 log_path 等值");

    live.commit(b"authoritative\n").expect("commit");
    assert_eq!(
        exec_output_log_path_in(&change_dir, live.exec_id()).as_deref(),
        Some(live.log_path()),
        "commit 后仍命中"
    );
}

/// 异常（参数化 7 + 2 形态）：未 begin 过的 change 目录 / 未知 exec_id / 非法
/// exec_id 七形态 → `None`。
#[test]
fn exec_output_log_path_in未命中与非法id均none() {
    let root = TempRoot::new("log-path-miss");
    let change_dir = change_dir_of(root.path(), "demo");
    let ghost_dir = change_dir_of(root.path(), "ghost-change");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    assert!(exec_output_log_path_in(&change_dir, live.exec_id()).is_some());

    for hostile in ["", "..", "../secret", "a/b", r"a\b", "C:evil", "exec-1.cmd"] {
        assert!(
            exec_output_log_path_in(&change_dir, hostile).is_none(),
            "非法 exec_id {hostile:?} → None"
        );
    }
    assert!(
        exec_output_log_path_in(&change_dir, "exec-404-0").is_none(),
        "未知 exec_id → None"
    );
    assert!(
        exec_output_log_path_in(&ghost_dir, live.exec_id()).is_none(),
        "未 begin 过的 change 目录 → None"
    );
}

/// 边界：同 id 在 Test 与 Baseline 两槽并存（人为双写）→ 返回 Test 槽路径
///（槽序 Test 先，首中即返；零串树）。
#[test]
fn exec_output_log_path_in双槽并存取test槽() {
    let root = TempRoot::new("log-path-dual");
    let change_dir = change_dir_of(root.path(), "demo");
    let exec_id = "exec-9-0";
    let test_dir = slot_dir(root.path(), "demo", ReportSlot::Test);
    let baseline_dir = slot_dir(root.path(), "demo", ReportSlot::Baseline);
    std::fs::create_dir_all(&test_dir).expect("建 test 目录");
    std::fs::create_dir_all(&baseline_dir).expect("建 baseline 目录");
    std::fs::write(test_dir.join(format!("{exec_id}.log")), b"from-test").expect("写 test");
    std::fs::write(
        baseline_dir.join(format!("{exec_id}.log")),
        b"from-baseline",
    )
    .expect("写 baseline");

    assert_eq!(
        exec_output_log_path_in(&change_dir, exec_id),
        Some(test_dir.join(format!("{exec_id}.log"))),
        "首锚 Test 先命中（零串树）"
    );
    assert_eq!(
        read_exec_command_in(&change_dir, exec_id),
        None,
        "两槽皆无 sidecar（只人为写了 .log）"
    );
}

/// 正向：`read_exec_output_delta_in` 的 `from_offset = 0` 与 `read_exec_output`
/// 逐字相等、`next_offset` == 该文本字节长度（AC-4）。
#[test]
fn read_exec_output_delta_in全量与整读逐字相等() {
    let root = TempRoot::new("delta-full");
    let text = "clippy: 3 warnings\n行二 🚀\n";
    let exec_id =
        record_exec_output(root.path(), "demo", ReportSlot::Test, text.as_bytes()).expect("落盘");
    let delta = read_exec_output_delta_in(&change_dir_of(root.path(), "demo"), &exec_id, 0)
        .expect("应命中");

    assert_eq!(delta.text, text, "全量与整读逐字相等");
    assert_eq!(delta.next_offset, text.len() as u64, "游标 == 字节长度");
    assert_eq!(
        Some(delta.text.clone().into_bytes()),
        read_exec_output(root.path(), "demo", &exec_id),
        "与 read_exec_output 逐字节一致"
    );
}

/// 正向：三次连续读（每次传上次 `next_offset`）→ 拼接逐字等于全量、游标单调
/// 不减、无新增内容时返回空串且游标不变。
#[test]
fn read_exec_output_delta_in连续增量拼接等于全量() {
    let root = TempRoot::new("delta-cont");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let sink = live.sink();

    let mut assembled = String::new();
    let mut offset = 0u64;
    let mut last = 0u64;
    for chunk in ["first\n", "second 🚀\n", "third\n"] {
        sink.absorb(chunk.as_bytes());
        let delta = read_exec_output_delta_in(&change_dir, live.exec_id(), offset).expect("应命中");
        assert!(delta.next_offset >= last, "游标单调不减");
        assembled.push_str(&delta.text);
        offset = delta.next_offset;
        last = delta.next_offset;
    }
    assert_eq!(assembled, "first\nsecond 🚀\nthird\n", "拼接逐字等于全量");

    let idle = read_exec_output_delta_in(&change_dir, live.exec_id(), offset).expect("应命中");
    assert_eq!(idle.text, "", "无新增 → 空串");
    assert_eq!(idle.next_offset, offset, "游标不变");
}

/// 异常：`from_offset > 文件长度`（覆盖变短）→ 夹 0 重读、返回全文且
/// `next_offset == 新长度`（MUST NOT panic / MUST NOT 返回空串）。
#[test]
fn read_exec_output_delta_in覆盖变短夹零重读() {
    let root = TempRoot::new("delta-shrink");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    live.sink().absorb(&[b'a'; 100]);
    let first = read_exec_output_delta_in(&change_dir, live.exec_id(), 0).expect("读 100 字节");
    assert_eq!(first.next_offset, 100);

    live.commit(&[b'b'; 20]).expect("覆盖为 20 字节");
    let after = read_exec_output_delta_in(&change_dir, live.exec_id(), 100).expect("夹 0 重读");
    assert_eq!(after.text.len(), 20, "返回全文（非空串）");
    assert_eq!(after.next_offset, 20, "夹 0 后游标 == 新长度");
    assert!(
        after.text.chars().all(|ch| ch == 'b'),
        "内容为覆盖后的权威内容"
    );
}

/// 异常（参数化 7 + 1 形态）：未 begin 的 exec_id / 非法 exec_id → `None`
///（miss 非错误）。
#[test]
fn read_exec_output_delta_in未命中与非法id均none() {
    let root = TempRoot::new("delta-miss");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    assert!(read_exec_output_delta_in(&change_dir, live.exec_id(), 0).is_some());

    assert!(
        read_exec_output_delta_in(&change_dir, "exec-404-0", 0).is_none(),
        "未 begin 的 exec_id → None"
    );
    for hostile in ["", "..", "../secret", "a/b", r"a\b", "C:evil", "exec-1.cmd"] {
        assert!(
            read_exec_output_delta_in(&change_dir, hostile, 0).is_none(),
            "非法 exec_id {hostile:?} → None"
        );
    }
}

/// 边界：块末半字符——追加 `"中"` 的前 2 字节 → 本次不产出该字符碎片、
/// `next_offset` 停在该字符起点；补齐第 3 字节后完整返回 `"中"`。
#[test]
fn read_exec_output_delta_in块末半字符留待下次() {
    let root = TempRoot::new("delta-half");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let sink = live.sink();
    sink.absorb(b"head");
    let zhong = "中".as_bytes();
    assert_eq!(zhong.len(), 3);
    sink.absorb(&zhong[..2]);

    let partial = read_exec_output_delta_in(&change_dir, live.exec_id(), 0).expect("读");
    assert_eq!(partial.text, "head", "不含半字符任何碎片");
    assert_eq!(partial.next_offset, 4, "游标停在该字符起点");

    sink.absorb(&zhong[2..]);
    let completed =
        read_exec_output_delta_in(&change_dir, live.exec_id(), partial.next_offset).expect("读");
    assert_eq!(completed.text, "中", "补齐后完整返回该字符");
    assert_eq!(completed.next_offset, 7, "游标落在字符边界（4 + 3）");
}

/// 边界：非法字节推进——追加 `b"head\xff\xfe"` → 以替换符呈现且游标严格越过
/// 非法字节（不形成永不推进循环）；继续追加合法文本后读到其后的合法文本。
#[test]
fn read_exec_output_delta_in非法字节不卡游标() {
    let root = TempRoot::new("delta-invalid");
    let change_dir = change_dir_of(root.path(), "demo");
    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let sink = live.sink();
    sink.absorb(b"head\xff\xfe");

    let delta = read_exec_output_delta_in(&change_dir, live.exec_id(), 0).expect("读");
    assert!(
        delta.text.starts_with("head"),
        "合法前缀原样保留: {:?}",
        delta.text
    );
    assert!(
        delta.text.contains('\u{FFFD}'),
        "非法字节以替换符呈现: {:?}",
        delta.text
    );
    assert!(
        delta.next_offset > 4,
        "游标严格越过非法字节（实际 {}）",
        delta.next_offset
    );

    sink.absorb(b"-tail\n");
    let next =
        read_exec_output_delta_in(&change_dir, live.exec_id(), delta.next_offset).expect("读");
    assert!(
        next.text.ends_with("-tail\n"),
        "后续合法文本可读: {:?}",
        next.text
    );
}

/// 边界（参数化 5 形态）：空文件立即读 → `Some { text: "", next_offset: 0 }`
///（与 miss 可分辨）；`from_offset` 取长度 / 长度+1 / `u64::MAX-1` /
/// `u64::MAX` → 夹 0 重读且零 panic。
#[test]
fn read_exec_output_delta_in空文件与游标极值() {
    let root = TempRoot::new("delta-empty");
    let change_dir = change_dir_of(root.path(), "demo");
    let fresh = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    let empty = read_exec_output_delta_in(&change_dir, fresh.exec_id(), 0).expect("空文件亦命中");
    assert_eq!(empty.text, "", "空文件返回空串");
    assert_eq!(empty.next_offset, 0, "游标 0");
    assert!(
        read_exec_output_delta_in(&change_dir, "exec-404-0", 0).is_none(),
        "空文件与 miss 可分辨"
    );

    let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
    live.commit(b"ab").expect("写两字节");
    let len = 2u64;
    for from in [0u64, len, len + 1, u64::MAX - 1, u64::MAX] {
        let delta = read_exec_output_delta_in(&change_dir, live.exec_id(), from)
            .unwrap_or_else(|| panic!("from_offset={from} 应命中"));
        if from > len {
            assert_eq!(delta.text, "ab", "from={from} 夹 0 重读全文");
            assert_eq!(delta.next_offset, len, "from={from} 游标 == 长度");
        } else if from == len {
            assert_eq!(delta.text, "", "from=len 无新增");
            assert_eq!(delta.next_offset, len, "from=len 游标不动");
        } else {
            assert_eq!(delta.text, "ab", "from={from} 全文");
            assert_eq!(delta.next_offset, len, "from={from} 游标 == 长度");
        }
    }
}

/// 边界（参数化 1 / 2 / 3 字节截断）：4 字节字符（`🚀`）跨块边界三形态均不
/// 产出碎片、补齐后完整返回；`next_offset` 恒落字符边界。
#[test]
fn read_exec_output_delta_in四字节字符跨块截断三形态() {
    let rocket = "🚀".as_bytes();
    assert_eq!(rocket.len(), 4);
    for cut in [1usize, 2, 3] {
        let root = TempRoot::new(&format!("delta-rocket-{cut}"));
        let change_dir = change_dir_of(root.path(), "demo");
        let live = LiveExec::begin(root.path(), "demo", ReportSlot::Test).expect("begin");
        let sink = live.sink();
        sink.absorb(&rocket[..cut]);

        let partial = read_exec_output_delta_in(&change_dir, live.exec_id(), 0)
            .unwrap_or_else(|| panic!("cut={cut} 应命中"));
        assert_eq!(partial.text, "", "cut={cut}: 不产出碎片");
        assert_eq!(partial.next_offset, 0, "cut={cut}: 游标停在该字符起点");

        sink.absorb(&rocket[cut..]);
        let completed = read_exec_output_delta_in(&change_dir, live.exec_id(), partial.next_offset)
            .expect("读");
        assert_eq!(completed.text, "🚀", "cut={cut}: 补齐后完整返回");
        assert_eq!(completed.next_offset, 4, "cut={cut}: 游标落字符边界");
    }
}

/// 正向：`LiveSink` 可 `Clone`、`Debug` 手写为 `"LiveSink(..)"`（`Arc<dyn Fn>`
/// 无 `Debug`，而 `RunOptions` 派生了 `Debug`）；crate 内直构后多次 `absorb`
/// 按到达序投递（D2）。
#[test]
fn live_sink_clone与手写debug与到达序投递() {
    let seen: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);
    let sink = LiveSink::new(move |bytes: &[u8]| {
        recorder.lock().expect("锁").push(bytes.to_vec());
    });
    let cloned = sink.clone();

    sink.absorb(b"first");
    cloned.absorb(b"second");

    assert_eq!(
        format!("{sink:?}"),
        "LiveSink(..)",
        "手写 Debug 输出定式（不含闭包内容）"
    );
    assert_eq!(format!("{cloned:?}"), "LiveSink(..)");
    assert_eq!(
        *seen.lock().expect("锁"),
        vec![b"first".to_vec(), b"second".to_vec()],
        "clone 共享同一闭包、按到达序投递"
    );
}

/// 异常：`LiveSink` 的闭包在 `absorb` 内部不改变投递语义——空 `&[]` 亦投递 1
/// 次、零 panic；`RunOptions` 的 `Debug` 派生在位（`format!("{:?}")` 不 panic，
/// 红线 ②）。
#[test]
fn live_sink空投递与run_options_debug派生() {
    let count = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&count);
    let sink = LiveSink::new(move |bytes: &[u8]| {
        assert!(bytes.is_empty(), "本次投递应为空片");
        *counter.lock().expect("锁") += 1;
    });
    sink.absorb(&[]);
    assert_eq!(*count.lock().expect("锁"), 1, "空片投递 1 次");

    let options = crate::RunOptions {
        cwd: None,
        env: Vec::new(),
        timeout: std::time::Duration::from_secs(1),
        cancel: foundation::cancel::CancelFlag::never(),
        retry: crate::RetryPolicy::none(),
        live: Some(sink.clone()),
    };
    let debug = format!("{options:?}");
    assert!(debug.contains("live"), "Debug 面含 live 字段: {debug}");
    assert!(debug.contains("LiveSink(..)"), "Debug 不泄漏闭包");
}

/// 边界：`ExecOutputDelta` 零 derive 的形态事实——仅字段可读、构造与解构零额
/// 外约束，`next_offset` 取 `0` / `u64::MAX` 可赋值（纯结构）。
#[test]
fn exec_output_delta字段面可读且零额外约束() {
    let zero = ExecOutputDelta {
        text: String::new(),
        next_offset: 0,
    };
    assert_eq!(zero.next_offset, 0);
    assert!(zero.text.is_empty());

    let huge = ExecOutputDelta {
        text: "🚀".to_owned(),
        next_offset: u64::MAX,
    };
    let ExecOutputDelta { text, next_offset } = huge;
    assert_eq!(text, "🚀");
    assert_eq!(next_offset, u64::MAX);
}
