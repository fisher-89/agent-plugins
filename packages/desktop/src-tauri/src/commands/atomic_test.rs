//! `commands::atomic` 的单元测试：命令层薄委托与唯一实现的**同一性**（行为等价、
//! 错误串原样透传、边界形态同行为），以及「命令层零落盘实现」的源码锚。
//!
//! 行为用例（写新 / 覆盖 / 空与超长 / 特殊名 / 父目录缺失 / 只读目录 / 结构锚）已迁
//! infra 侧 `crates/infra/agent/src/sdk/atomic_test.rs`（内容逐条保留）；本文件只钉
//! 「委托目标 = 被测对象」这一条——替身会抹掉「两写面共用同一函数」的判据。
//!
//! 文件系统不 mock：真实 `tempfile` RAII + 真实 infra 实现（经 crate 根
//! `agent_runtime::write_atomic`）。

use std::fs;
use std::path::Path;

use super::write_atomic;

/// 临时根（RAII 清理）。
fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("command-atomic-test-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败")
}

/// 目录内 `*.archi-tmp` 残留条目。
fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("读目录失败")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".archi-tmp"))
        .collect()
}

// ---------------------------------------------------------------------------
// 正向：委托后行为等价
// ---------------------------------------------------------------------------

/// 正向（矩阵 2 形态）：写新文件 / 覆盖既有文件 ⇒ 读回逐字等于入参、同目录零
/// `*.archi-tmp` 残留（与 infra 实现同行为）。
#[test]
fn 委托后行为等价() {
    let dir = tempdir("equivalent");
    let fresh = dir.path().join("gate.toml");
    write_atomic(&fresh, "gate = \"strict\"\n").expect("写新文件应成功");
    assert_eq!(
        fs::read_to_string(&fresh).expect("读目标失败"),
        "gate = \"strict\"\n",
        "写新：内容逐字等于入参"
    );

    let existing = dir.path().join("existing.toml");
    fs::write(&existing, "旧内容：很长很长的一段旧模型字节\n").expect("落旧内容失败");
    write_atomic(&existing, "新内容\n").expect("覆盖应成功");
    assert_eq!(
        fs::read_to_string(&existing).expect("读目标失败"),
        "新内容\n",
        "覆盖：整体替换（无半截）"
    );
    assert!(leftovers(dir.path()).is_empty(), "零临时文件残留");
}

// ---------------------------------------------------------------------------
// 异常：错误串原样透传
// ---------------------------------------------------------------------------

/// 异常（矩阵 2 形态）：父目录缺失 ⇒ `Err` 与 `agent_runtime::write_atomic` 直调**逐字
/// 相等**；空路径 ⇒ 「无父目录」。
#[test]
fn 错误文案原样透传() {
    let dir = tempdir("passthrough");
    let target = dir.path().join("absent-dir").join("gate.toml");

    let delegated = write_atomic(&target, "x").expect_err("父目录缺失必须 Err");
    let direct = agent_runtime::write_atomic(&target, "x").expect_err("直调同判据");
    assert_eq!(delegated, direct, "错误串逐字透传（同一实现）");
    assert!(
        delegated.contains("写入临时文件失败"),
        "文案仍是 infra 侧那句：{delegated}"
    );
    assert!(!target.exists(), "目标未被创建");

    let empty = write_atomic(Path::new(""), "x").expect_err("空路径必须 Err");
    assert!(
        empty.contains("无父目录"),
        "空路径文案含「无父目录」：{empty}"
    );
}

// ---------------------------------------------------------------------------
// 边界：边界形态与 infra 同行为
// ---------------------------------------------------------------------------

/// 边界（矩阵 3 形态）：空内容 / 1 MB 超长内容 / 目标名以 `.archi-tmp` 结尾 ⇒ 委托后与
/// `agent_runtime::write_atomic` 直调同行为（读回内容相等、零临时残留、后缀不变）。
#[test]
fn 边界形态与infra同行为() {
    let dir = tempdir("boundary");
    let long = "x".repeat(1024 * 1024);
    for (name, content) in [
        ("empty.toml", ""),
        ("big.toml", long.as_str()),
        ("already.archi-tmp", "suffix"),
    ] {
        let delegated = dir.path().join(format!("delegated-{name}"));
        let direct = dir.path().join(format!("direct-{name}"));
        write_atomic(&delegated, content).expect("委托写入应成功");
        agent_runtime::write_atomic(&direct, content).expect("直调写入应成功");
        assert_eq!(
            fs::read(&delegated).expect("读委托产物失败"),
            fs::read(&direct).expect("读直调产物失败"),
            "形态 {name} 的字节与直调逐字相等"
        );
        assert_eq!(
            fs::metadata(&delegated).expect("读元数据失败").len() as usize,
            content.len(),
            "形态 {name} 的长度一致"
        );
    }
    assert_eq!(
        leftovers(dir.path()),
        vec![
            "delegated-already.archi-tmp".to_owned(),
            "direct-already.archi-tmp".to_owned(),
        ],
        "仅那两个合法目标名本身残留（后缀不变、临时名未撞目标）"
    );
}

// ---------------------------------------------------------------------------
// 结构锚：命令层零落盘实现
// ---------------------------------------------------------------------------

/// 异常（源码锚）：`fs::write` / `fs::rename` / `create_dir_all` 零命中、
/// `agent_runtime::write_atomic` 恰一处（委托单点）、签名行逐字在场。
#[test]
fn 命令层零落盘实现() {
    // 只扫代码行：模块头 doc 明写「本文件零 `fs::write` …」正是本判据的说明文字。
    let own = include_str!("atomic.rs");
    let code: Vec<&str> = own
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    for forbidden in ["fs::write", "fs::rename", "create_dir_all"] {
        assert!(
            !code.iter().any(|line| line.contains(forbidden)),
            "命令层零 {forbidden}"
        );
    }
    assert_eq!(
        code.iter()
            .map(|line| line.matches("agent_runtime::write_atomic").count())
            .sum::<usize>(),
        1,
        "委托单点恰一处"
    );
    assert!(
        code.iter().any(|line| line.contains(
            "pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>"
        )),
        "签名行逐字不变（壳层两写面共用）"
    );
}
