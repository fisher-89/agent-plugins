//! `sdk::atomic` 的单元测试：全仓**唯一落盘实现** `write_atomic` 的同目录临时文件 +
//! rename 覆盖语义（写新 / 覆盖 / 形态边界 / 失败清理 / 只读目录），以及「唯一落盘点 /
//! 零建目录 / 签名与壳层委托面一致」三条锚。
//!
//! 本文件是壳层 `src/commands/atomic_test.rs` 行为用例的**迁移落点**（内容逐条保留，
//! 壳层只留薄委托断言）。文件系统不 mock：原子性（临时文件 + rename）、失败清临时文件、
//! 错误文案都是被测语义，替身会抹掉判定对象；真实 `tempfile` RAII 装置（infra/agent 既有
//! dev-dependency，零新增）。

use std::fs;
use std::path::Path;

use super::atomic::write_atomic;

/// 临时根（RAII 清理）。
fn tempdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("atomic-write-test-{tag}-"))
        .tempdir()
        .expect("创建临时目录失败")
}

/// 目录内 `*.archi-tmp` 残留条目（含同后缀命名的合法目标名时按入参区分）。
fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("读目录失败")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".archi-tmp"))
        .collect()
}

// ---------------------------------------------------------------------------
// 正向：写新 / 覆盖
// ---------------------------------------------------------------------------

/// 正向：写新文件 → 内容逐字等于入参、同目录零 `*.archi-tmp` 残留。
#[test]
fn 写新文件内容逐字且零残留() {
    let dir = tempdir("new");
    let target = dir.path().join("gate.toml");

    write_atomic(&target, "gate = \"strict\"\n").expect("写新文件应成功");

    assert_eq!(
        fs::read_to_string(&target).expect("读目标失败"),
        "gate = \"strict\"\n",
        "内容逐字等于入参"
    );
    assert!(leftovers(dir.path()).is_empty(), "同目录零临时文件残留");
}

/// 正向：覆盖既有文件 → 内容整体替换（读回逐字等于新内容，无半截）。
#[test]
fn 覆盖既有文件为整体替换() {
    let dir = tempdir("overwrite");
    let target = dir.path().join("gate.toml");
    fs::write(&target, "旧内容：很长很长的一段旧模型字节\n第二行\n").expect("落旧内容失败");

    write_atomic(&target, "新内容\n").expect("覆盖应成功");

    assert_eq!(
        fs::read_to_string(&target).expect("读目标失败"),
        "新内容\n",
        "覆盖后无旧字节残留（整体替换）"
    );
    assert!(leftovers(dir.path()).is_empty(), "覆盖后零临时文件残留");
}

// ---------------------------------------------------------------------------
// 边界：空内容 / 超长内容 / 无父目录 / 特殊名字
// ---------------------------------------------------------------------------

/// 边界（矩阵 2 形态）：空内容 → `Ok` 且文件为空；1 MB 内容 → `Ok` 且读回逐字相等。
#[test]
fn 空内容与超长内容两形态() {
    let dir = tempdir("size");
    let empty = dir.path().join("empty.toml");
    write_atomic(&empty, "").expect("空内容应成功");
    assert_eq!(
        fs::metadata(&empty).expect("读元数据失败").len(),
        0,
        "文件为空"
    );

    let big = dir.path().join("big.toml");
    let content = "x".repeat(1024 * 1024);
    write_atomic(&big, &content).expect("1 MB 内容应成功");
    assert_eq!(
        fs::read_to_string(&big).expect("读大文件失败").len(),
        content.len(),
        "读回逐字相等（长度一致）"
    );
    assert!(leftovers(dir.path()).is_empty(), "零临时文件残留");
}

/// 边界：目标路径无父目录（空路径）→ `Err` 文案含「无父目录」。
///
/// 判据边界说明：`Path::new("name").parent()` 是 `Some("")`（相对名按进程 cwd
/// 解析，写入成功），故该守卫的实际触发面只有空路径形态——本用例钉的正是这一面，
/// 不为「纯文件名」造一条不存在的失败。
#[test]
fn 无父目录目标err文案可辨() {
    let error = write_atomic(Path::new(""), "x").expect_err("空路径必须 Err");
    assert!(
        error.contains("无父目录"),
        "文案应点明无父目录，实际 {error}"
    );
}

/// 边界（矩阵 3 形态）：目标名含空格 / emoji / 名字本身以 `.archi-tmp` 结尾 →
/// 临时名不撞目标、写入成功。
#[test]
fn 特殊目标名临时名不撞目标() {
    let dir = tempdir("special-name");
    for name in ["含 空格.toml", "emoji-🎯.toml", "already.archi-tmp"] {
        let target = dir.path().join(name);
        write_atomic(&target, name).expect("特殊名应写入成功");
        assert_eq!(
            fs::read_to_string(&target).expect("读目标失败"),
            name,
            "目标名 {name:?} 内容逐字"
        );
    }
    // 后缀为 .archi-tmp 的合法目标本身不被本函数的残留清理判据误判：
    // 除该目标外无其他临时文件残留。
    let remaining = leftovers(dir.path());
    assert_eq!(
        remaining,
        vec!["already.archi-tmp".to_owned()],
        "仅有那个合法目标名本身（临时名未撞目标、未残留）"
    );
}

// ---------------------------------------------------------------------------
// 异常：父目录不存在 / 目录只读
// ---------------------------------------------------------------------------

/// 异常：父目录不存在 → `Err` 文案含「写入临时文件失败」，零残留、目标未被创建。
#[test]
fn 父目录不存在err且目标未创建() {
    let dir = tempdir("missing-parent");
    let target = dir.path().join("absent-dir").join("gate.toml");

    let error = write_atomic(&target, "x").expect_err("父目录缺失必须 Err");

    assert!(
        error.contains("写入临时文件失败"),
        "文案应点明写入临时文件失败，实际 {error}"
    );
    assert!(!target.exists(), "目标未被创建");
    assert!(
        !dir.path().join("absent-dir").exists(),
        "父目录不被自动创建（本函数零 create_dir_all）"
    );
    assert!(leftovers(dir.path()).is_empty(), "零残留");
}

/// 异常：目录只读（`cfg(unix)` 0o555）→ `Err`；以 root 运行时可写穿时只断言
/// 零残留 + 目标可读（沿既有 `archi` 兜底口径，不伪造失败）。
#[cfg(unix)]
#[test]
fn 只读目录err或可写穿零残留() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir("readonly");
    let target = dir.path().join("gate.toml");
    fs::write(&target, "旧内容").expect("落旧内容失败");
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).expect("置只读");

    let result = write_atomic(&target, "新内容");
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).expect("恢复权限");

    if result.is_ok() {
        assert!(leftovers(dir.path()).is_empty(), "可写穿时零临时文件残留");
        assert!(fs::read(&target).is_ok(), "目标文件仍可读");
        return;
    }
    assert_eq!(
        fs::read_to_string(&target).expect("读目标失败"),
        "旧内容",
        "失败时原文件不变"
    );
    assert!(leftovers(dir.path()).is_empty(), "失败后零临时文件残留");
}

// ---------------------------------------------------------------------------
// 结构锚 / 签名锚
// ---------------------------------------------------------------------------

/// 异常（源码锚）：`fs::write(&temporary` / `fs::rename(&temporary, path)` 各恰一处、
/// `create_dir_all` 零命中、`.archi-tmp` 恰一处（后缀单点）。
#[test]
fn 结构锚唯一落盘点与零建目录() {
    let source = include_str!("atomic.rs");
    assert_eq!(
        source.matches("fs::write(&temporary").count(),
        1,
        "全仓唯一 `fs::write` 落点（临时文件写入）"
    );
    assert_eq!(
        source.matches("fs::rename(&temporary, path)").count(),
        1,
        "rename 覆盖恰一处"
    );
    assert!(
        !source.contains("create_dir_all"),
        "本函数不建目录（父目录缺失即 Err）"
    );
    assert_eq!(
        source.matches("format!(\"{name}.archi-tmp\")").count(),
        1,
        "临时后缀恰一处（doc 之外的唯一构造点）"
    );
}

/// 边界（签名锚）：函数指针形态即编译期判据——签名与可见性与壳层委托面逐字一致
/// （改签名即编译失败）。
#[test]
fn 签名与壳层委托面一致() {
    let function: fn(&Path, &str) -> Result<(), String> = write_atomic;
    let dir = tempdir("signature");
    let target = dir.path().join("gate.toml");
    function(&target, "gate = \"off\"\n").expect("经函数指针写入应成功");
    assert_eq!(
        fs::read_to_string(&target).expect("读目标失败"),
        "gate = \"off\"\n",
        "函数指针调用即真实落盘"
    );
}

// ---------------------------------------------------------------------------
// 模块登记面（`sdk/mod.rs` 的机械连带）
// ---------------------------------------------------------------------------

/// 正向：`sdk/mod.rs` 内 `pub(crate) mod atomic;` 恰一处、`#[cfg(test)] mod atomic_test;`
/// 恰一处（登记 = 本文件能编译即成立的可执行证据）。
#[test]
fn atomic模块登记在册() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("pub(crate) mod atomic;").count(),
        1,
        "模块声明恰一处"
    );
    assert_eq!(
        source.matches("mod atomic_test;").count(),
        1,
        "测试登记恰一处"
    );
    assert!(
        source.contains(
            "#[cfg(test)]
mod atomic_test;"
        ),
        "测试登记带 `#[cfg(test)]` 门"
    );
}

/// 异常（矩阵 2 形态）：可见性维持 `pub(crate)`——`pub mod atomic` 与 `mod atomic;`
/// 两形态零命中。
#[test]
fn 模块可见性维持pub_crate() {
    let source = include_str!("mod.rs");
    assert!(!source.contains("pub mod atomic"), "零提升为公开模块");
    assert!(
        !source.lines().any(|line| line.trim() == "mod atomic;"),
        "零私有到 `sdk` 的声明（装配面需要 `pub(crate)`）"
    );
}

/// 边界：声明单点——`atomic` 模块只在 `sdk/mod.rs` 声明一次，且声明块内插在模块区
/// （测试登记区与模块区各一处，互不混写）。
#[test]
fn 声明单点() {
    let source = include_str!("mod.rs");
    assert_eq!(
        source.matches("mod atomic").count(),
        2,
        "仅两处（模块声明 + 测试登记）"
    );
    let declaration = source.find("pub(crate) mod atomic;").expect("模块声明在案");
    let test_block = source
        .find(
            "#[cfg(test)]
mod archi_tool_test;",
        )
        .expect("测试登记区在案");
    assert!(
        declaration < test_block,
        "模块声明在模块区（测试登记区之前）"
    );
}
