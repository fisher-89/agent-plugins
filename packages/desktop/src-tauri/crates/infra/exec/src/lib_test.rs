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

use crate::{read_exec_output, record_exec_output};

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

    let exec_id = record_exec_output(root.path(), "demo-change", &output).expect("落盘应成功");
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
    let first = record_exec_output(root.path(), "change-a", b"one").expect("落盘应成功");
    let second = record_exec_output(root.path(), "change-a", b"two").expect("落盘应成功");
    assert_ne!(first, second, "两次铸造 exec_id 互异");

    let third = record_exec_output(root.path(), "change-b", b"three").expect("落盘应成功");
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
    let exec_id = record_exec_output(root.path(), "demo", &output).expect("落盘应成功");

    let read_back = read_exec_output(root.path(), "demo", &exec_id).expect("回查应命中");
    assert_eq!(read_back, output);
    // 重复回读恒同字节（读面无副作用）
    assert_eq!(
        read_exec_output(root.path(), "demo", &exec_id).expect("重复回查应命中"),
        output
    );
}

/// 异常：未知 exec_id / 非法形态（空串 / `..` / 路径分隔符 / 盘符 / `.log`
/// 后缀）→ None（miss 非错误，且零路径逃逸）。
#[test]
fn read_exec_output未知与非法exec_id返回none() {
    let root = TempRoot::new("miss");
    let exec_id = record_exec_output(root.path(), "demo", b"payload").expect("落盘应成功");

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
    let empty = record_exec_output(root.path(), "demo", b"").expect("空输出落盘应成功");
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
    let exec_id = record_exec_output(root.path(), "demo", &long).expect("超长落盘应成功");
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

    let err = record_exec_output(root.path(), "demo", b"stdout").expect_err("应 Err");
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
        let exec_id = record_exec_output(root.path(), "demo", &output).expect("落盘应成功");
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
