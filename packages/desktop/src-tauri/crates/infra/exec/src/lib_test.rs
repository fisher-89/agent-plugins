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
// 输出记录单点，AC-8）
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
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 正向：落盘写入 change 报告目录树（`exec-outputs/<exec_id>.log`），返回非空
/// `exec_id`；回查逐字节一致；stderr 另文件保真（stdout / stderr 分文件）。
#[test]
fn record_exec_output落盘并回查逐字节一致() {
    let root = TempRoot::new("roundtrip");
    let stdout = b"clippy: warning: unused import\n".to_vec();
    let stderr = b"error[E0308]: mismatched types\n".to_vec();

    let exec_id =
        record_exec_output(root.path(), "demo-change", &stdout, &stderr).expect("落盘应成功");
    assert!(!exec_id.is_empty(), "exec_id 非空");
    assert!(
        exec_id.starts_with("exec-"),
        "铸造定式 exec-<millis>-<序号>: {exec_id}"
    );

    let read_back = read_exec_output(root.path(), "demo-change", &exec_id).expect("回查应命中");
    assert_eq!(read_back, stdout, "回查全量字节与写入一致");
    let log_path = root
        .path()
        .join("openspec/changes/demo-change/reports/test/exec-outputs")
        .join(format!("{exec_id}.log"));
    let fallback = foundation::layout::change_test_reports(root.path(), "demo-change")
        .join("exec-outputs")
        .join(format!("{exec_id}.log"));
    assert!(
        log_path.exists() || fallback.exists(),
        "落盘位于报告目录树 exec-outputs/<exec_id>.log"
    );
    let err_path = fallback
        .parent()
        .expect("父子目录")
        .join(format!("{exec_id}.err"));
    assert_eq!(
        std::fs::read(&err_path).expect("stderr 分文件应可读"),
        stderr,
        "stderr 分文件保真"
    );
}

/// 正向：两次落盘 exec_id 互异（引用唯一面）；change 名隔离（不同 change 目录
/// 各自回查）。
#[test]
fn record_exec_output两次铸造互异且change隔离() {
    let root = TempRoot::new("unique");
    let first = record_exec_output(root.path(), "change-a", b"one", b"").expect("落盘应成功");
    let second = record_exec_output(root.path(), "change-a", b"two", b"").expect("落盘应成功");
    assert_ne!(first, second, "两次铸造 exec_id 互异");

    let third = record_exec_output(root.path(), "change-b", b"three", b"").expect("落盘应成功");
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

/// 边界：stdout / stderr 全空 → 落盘空内容仍可回查（`Some(vec![])`）；超长 +
/// 特殊字节（`\n` / `\0` / emoji）全量保真（引用式不截断）。
#[test]
fn record_exec_output空与超长特殊字节保真() {
    let root = TempRoot::new("bytes");
    let empty = record_exec_output(root.path(), "demo", b"", b"").expect("空输出落盘应成功");
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
    let exec_id = record_exec_output(root.path(), "demo", &long, &[]).expect("超长落盘应成功");
    let read_back = read_exec_output(root.path(), "demo", &exec_id).expect("回查应命中");
    assert_eq!(read_back.len(), long.len(), "超长全量保真（不截断）");
    assert_eq!(read_back, long, "含 \\0 与 emoji 字节逐字保真");
}

/// 异常：未知 exec_id / 非法形态（空串 / `..` / 路径分隔符 / 盘符）→ None
///（miss 非错误，且零路径逃逸）；落盘目录不可写（父路径被文件占位）→ Err 串
/// 语义。
#[test]
fn read_exec_output未知与非法exec_id返回none() {
    let root = TempRoot::new("miss");
    let exec_id = record_exec_output(root.path(), "demo", b"payload", b"").expect("落盘应成功");

    assert!(read_exec_output(root.path(), "demo", "exec-unknown").is_none());
    for hostile in ["", "..", "../secret", "a/b", "a\\b", "C:evil", "exec-1.log"] {
        assert!(
            read_exec_output(root.path(), "demo", hostile).is_none(),
            "非法 exec_id {hostile:?} 应拒绝"
        );
    }
    assert!(read_exec_output(root.path(), "demo", &exec_id).is_some());
}

/// 异常：落盘目录不可建（报告树路径被文件占位）→ `Err(String)` 串语义（不静
/// 默吞失败）。
#[test]
fn record_exec_output目录不可写时err串语义() {
    let root = TempRoot::new("unwritable");
    let reports = foundation::layout::change_test_reports(root.path(), "demo");
    std::fs::create_dir_all(reports.parent().expect("父目录")).expect("创建父目录失败");
    std::fs::write(&reports, "占位文件").expect("占位写失败");

    let err = record_exec_output(root.path(), "demo", b"stdout", b"").expect_err("应 Err");
    assert!(
        err.contains("exec") || err.contains("目录") || err.contains("落盘"),
        "Err 串语境保留: {err}"
    );
}
