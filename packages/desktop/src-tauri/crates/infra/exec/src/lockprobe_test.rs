//! 锁占用诊断探针测试：零 mock 系统——「另一进程持有」的等价面由**本进程真实
//! `std::fs::OpenOptions`（`share_mode` 两形态）** 扮演，判据观测的就是真实
//! `ERROR_SHARING_VIOLATION`；点名经真实 Restart Manager（Windows）/ `lsof`
//! 降级（非 Windows）。
//!
//! 平台隔离（R-8）：判据与 Restart Manager 点名用例以 `#[cfg(windows)]` 门控；
//! 非 Windows 只测「探针恒 `false`」与「`lsof` 缺失降级」两臂。

use std::path::{Path, PathBuf};

use super::blocks_delete;
use super::{attribution_text, lock_holders, locked_paths, LockHolder};

// ---------------------------------------------------------------------------
// 装置：真实临时文件（持有者句柄 / 只读自证 / 不存在路径）
// ---------------------------------------------------------------------------

/// 临时 workspace（真实落盘文件 + 目录引用）。
struct ProbeWs {
    dir: tempfile::TempDir,
}

impl ProbeWs {
    fn new(tag: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("lockprobe-{tag}-"))
            .tempdir()
            .expect("创建临时 workspace 失败");
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// 落盘一个带正文的文件并返回其路径（正文用于「只读不扰动」自证）。
    fn file(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path().join(name);
        std::fs::write(&path, body).expect("写临时文件失败");
        path
    }

    /// 永不存在的路径（点名 / 判据的 NotFound 臂）。
    fn missing(&self, name: &str) -> PathBuf {
        self.path().join("no-such-dir").join(name)
    }
}

/// 「另一进程持有」的本进程等价面（Windows）：`share` 由调用方给两形态；句柄存活
/// 期间判据/点名观察到共享冲突，`drop` 即释放。
#[cfg(windows)]
fn hold_with_share(path: &Path, share: u32) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(share)
        .open(path)
        .expect("以指定 share 模式打开临时文件失败")
}

/// `FILE_SHARE_READ`（无 delete 共享——与 rustc / cargo 失败的持有形态同条件）。
#[cfg(windows)]
const SHARE_READ_ONLY: u32 = 0x1;
/// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`（全共享——正常读句柄）。
#[cfg(windows)]
const SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;

// ---------------------------------------------------------------------------
// 正向：三形态路径解析
// ---------------------------------------------------------------------------

/// 正向（表驱动 3 形态）：Windows 路径三错误串形态各解析出 1 条对应路径，分隔符
/// 按输入原样保留（反斜杠不归一）。
#[test]
fn 三形态解析_反斜杠原样保留() {
    let cases: [(&str, &str); 3] = [
        (
            "error: failed to rename archive file: D:\\w\\target\\debug\\deps\\liba.rlib",
            "D:\\w\\target\\debug\\deps\\liba.rlib",
        ),
        (
            "error: failed to write D:\\w\\target\\libb.rmeta",
            "D:\\w\\target\\libb.rmeta",
        ),
        (
            "error: failed to remove D:\\w\\target\\c.rcgu.o",
            "D:\\w\\target\\c.rcgu.o",
        ),
    ];

    for (diagnostics, expected) in cases {
        let paths = locked_paths(diagnostics);
        assert_eq!(
            paths.len(),
            1,
            "形态 {diagnostics:?} 恰解析出 1 条路径: {paths:?}"
        );
        assert_eq!(
            paths[0],
            PathBuf::from(expected),
            "分隔符按输入原样保留（{diagnostics:?}）"
        );
    }
}

/// 正向（表驱动 3 形态）：POSIX 路径三形态各一 → 路径原样（正斜杠保留），无多余
/// 条目。
#[test]
fn posix形态解析_正斜杠原样保留() {
    let cases: [(&str, &str); 3] = [
        (
            "error: failed to rename archive file: /home/u/w/target/debug/build/x/out/liby.rlib",
            "/home/u/w/target/debug/build/x/out/liby.rlib",
        ),
        (
            "error: failed to write /home/u/w/target/libz.rmeta",
            "/home/u/w/target/libz.rmeta",
        ),
        (
            "error: failed to remove /home/u/w/target/obj/a.rcgu.o",
            "/home/u/w/target/obj/a.rcgu.o",
        ),
    ];

    for (diagnostics, expected) in cases {
        let paths = locked_paths(diagnostics);
        assert_eq!(
            paths,
            vec![PathBuf::from(expected)],
            "{diagnostics:?} 路径原样"
        );
    }
}

/// 正向（表驱动 2 形态，D9）：行尾噪声截断——只在首个 `": "`（冒号 + 空格）截断；
/// 括注形态 `(os error 32)` 亦剥净。
#[test]
fn 行尾噪声在首个冒号空格处截断() {
    let paths = locked_paths("error: failed to rename archive file: D:\\w\\liba.rlib: os error 32");
    assert_eq!(
        paths,
        vec![PathBuf::from("D:\\w\\liba.rlib")],
        "末尾 `: os error 32` 被首个 \": \" 规则截掉"
    );

    let paths = locked_paths("error: failed to write D:\\w\\libb.rmeta (os error 32)");
    assert_eq!(
        paths,
        vec![PathBuf::from("D:\\w\\libb.rmeta")],
        "尾随括注（内含 error）被剥离"
    );
}

/// 回归：编译诊断引用的源码行不参与路径解析——clippy / rustc 会把出错源码行
/// 渲染进诊断，其中的 `failed to rename archive file` 后跟的是 fixture 文本里的
/// 路径；真实错误行照常解析。
#[test]
fn 源码摘录行不参与路径解析() {
    let quoted = "error[E0061]: this function takes 4 arguments but 3 arguments were supplied\n    |\n287 |         b\"error: failed to rename archive file: /t/liba.rlib: os error 32\",\n    |\n";
    let paths = locked_paths(quoted);
    assert!(paths.is_empty(), "源码摘录行不产路径: {paths:?}");
    assert_eq!(attribution_text(quoted), None, "无真实锁证据时不产归因文本");

    let mixed = format!("{quoted}error: failed to write /t/real.rmeta: os error 32\n");
    assert_eq!(
        locked_paths(&mixed),
        vec![PathBuf::from("/t/real.rmeta")],
        "真实错误行照常解析"
    );
}

// ---------------------------------------------------------------------------
// 边界：盘符冒号 / 包裹引号 / 大小写 / 去重 / 上限
// ---------------------------------------------------------------------------

/// 边界：盘符冒号不误截——`C:` 后是反斜杠而非空格，不被 `": "` 规则切断。
#[test]
fn 盘符冒号不误截() {
    let paths = locked_paths("error: failed to write C:\\Users\\w\\target\\lib.rmeta");
    assert_eq!(
        paths,
        vec![PathBuf::from("C:\\Users\\w\\target\\lib.rmeta")],
        "盘符后的冒号（后接反斜杠）不触发截断"
    );
}

/// 边界（表驱动 2 形态）：包裹引号与反引号——剥引号后取裸路径。
#[test]
fn 包裹引号与反引号被剥离() {
    let paths = locked_paths("error: failed to remove `D:\\w\\target\\a.rcgu.o`");
    assert_eq!(
        paths,
        vec![PathBuf::from("D:\\w\\target\\a.rcgu.o")],
        "反引号剥离"
    );

    let paths = locked_paths("error: failed to write \"D:\\w\\target\\b.rmeta\"");
    assert_eq!(
        paths,
        vec![PathBuf::from("D:\\w\\target\\b.rmeta")],
        "双引号剥离"
    );
}

/// 边界（表驱动 2 形态）：大小写不敏感——全大写 / 首字母大写形态与全小写等价解析。
#[test]
fn 关键词大小写不敏感() {
    let upper = locked_paths("FAILED TO RENAME ARCHIVE FILE: D:\\w\\liba.rlib");
    assert_eq!(
        upper,
        vec![PathBuf::from("D:\\w\\liba.rlib")],
        "全大写形态等价解析"
    );

    let title = locked_paths("Failed To Write D:\\w\\libb.rmeta");
    assert_eq!(
        title,
        vec![PathBuf::from("D:\\w\\libb.rmeta")],
        "首字母大写形态等价解析"
    );
}

/// 边界：多路径去重与顺序——3 条不同路径 + 1 条重复 → 3 条产出，顺序 = 首次
/// 出现序。
#[test]
fn 多路径去重且保首次出现序() {
    let diagnostics = "error: failed to write /t/a.rmeta\nerror: failed to write /t/b.rmeta\nerror: failed to write /t/a.rmeta\nerror: failed to remove /t/c.rcgu.o";
    let paths = locked_paths(diagnostics);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/t/a.rmeta"),
            PathBuf::from("/t/b.rmeta"),
            PathBuf::from("/t/c.rcgu.o"),
        ],
        "重复路径去重且保首次出现序"
    );
}

/// 边界（D9）：上限 8 条——11 条不同路径 → 恰 8 条产出（不 panic）。
#[test]
fn 路径上限八条() {
    let diagnostics = (0..11)
        .map(|index| format!("error: failed to write /t/p{index}.rmeta"))
        .collect::<Vec<_>>()
        .join("\n");

    let paths = locked_paths(&diagnostics);
    assert_eq!(paths.len(), 8, "上限 8 条（实际 {paths:?}）");
    assert_eq!(
        paths[0],
        PathBuf::from("/t/p0.rmeta"),
        "截断保首次出现序（前 8 条）"
    );
    assert_eq!(paths[7], PathBuf::from("/t/p7.rmeta"), "第 8 条在案");
}

// ---------------------------------------------------------------------------
// 异常：空与无匹配
// ---------------------------------------------------------------------------

/// 异常（表驱动 4 形态）：空串 / 全空白 / 无关键词的普通报错 / 仅关键词无路径
/// → 均空（非空判据挡住）。
#[test]
fn 空与无匹配四形态皆空() {
    for (label, diagnostics) in [
        ("空串", ""),
        ("全空白", "   \n\t\n  "),
        ("无关键词", "error[E0308]: mismatched types"),
        ("仅关键词无路径", "error: failed to write \n"),
    ] {
        let paths = locked_paths(diagnostics);
        assert!(paths.is_empty(), "{label} 应解析出空清单: {paths:?}");
    }

    let colon_only = locked_paths("error: failed to remove: ");
    assert!(colon_only.is_empty(), "仅关键词 + 冒号无路径 → 空");
}

// ---------------------------------------------------------------------------
// 边界：超长与特殊字符诊断
// ---------------------------------------------------------------------------

/// 边界（表驱动 3 形态）：>1000 字符诊断（中段含路径）仍解析；路径含空格 /
/// emoji / 中文 → 原样保留、不 panic。
#[test]
fn 超长与特殊字符诊断() {
    let long = format!(
        "{}\nerror: failed to write /t/keep.rmeta\n{}",
        "noise ".repeat(200),
        "tail ".repeat(200)
    );
    assert_eq!(
        locked_paths(&long),
        vec![PathBuf::from("/t/keep.rmeta")],
        "超长诊断中段的路径仍可解析"
    );

    let spaced = locked_paths("error: failed to write /t/my dir/lib b.rmeta");
    assert_eq!(
        spaced,
        vec![PathBuf::from("/t/my dir/lib b.rmeta")],
        "路径含空格原样保留"
    );

    let cjk = locked_paths("error: failed to write /t/构建目录/🎉产物.rmeta");
    assert_eq!(
        cjk,
        vec![PathBuf::from("/t/构建目录/🎉产物.rmeta")],
        "中文 / emoji 路径原样保留"
    );

    // 含 NUL 的诊断不 panic（路径片段原样承载）
    let with_nul = locked_paths("error: failed to write /t/a\u{0}b.rmeta");
    assert_eq!(with_nul.len(), 1, "含 NUL 的诊断不 panic: {with_nul:?}");
}

// ---------------------------------------------------------------------------
// 正向 / 边界：blocks_delete 判据（Windows 强制共享语义）
// ---------------------------------------------------------------------------

/// 正向（AC-4）：全共享读句柄不误报——另以「读权限 + 全共享」打开 → 判据
/// `false`（旧「独占打开探测」把正常读句柄误报成锁的形态不复现）。
#[cfg(windows)]
#[test]
fn 全共享读句柄不误报() {
    let ws = ProbeWs::new("share-all");
    let path = ws.file("lib.rmeta", "body");
    let holder = hold_with_share(&path, SHARE_ALL);

    assert!(
        !blocks_delete(&path),
        "全共享（含 DELETE）持有者不阻塞删除，判据不得误报"
    );
    drop(holder);
}

/// 正向（AC-4）：无 delete 共享即真阻塞——另以「读权限 + `share = READ`」打开
/// → 判据 `true`（`raw_os_error == 32` 判据，与 rustc / cargo 失败同条件）。
#[cfg(windows)]
#[test]
fn 无delete共享即真阻塞() {
    let ws = ProbeWs::new("share-read");
    let path = ws.file("lib.rmeta", "body");
    let holder = hold_with_share(&path, SHARE_READ_ONLY);

    assert!(
        blocks_delete(&path),
        "持有者不允许 DELETE 共享 → 判据为真（与 cargo 失败同条件）"
    );
    drop(holder);
    assert!(!blocks_delete(&path), "释放句柄后即判未阻塞");
}

/// 边界（表驱动 2 形态）：不存在路径 → `false`（`NotFound`）；无任何持有者的
/// 真实文件 → `false`（打开成功）。
#[cfg(windows)]
#[test]
fn 不存在与无持有者皆判未阻塞() {
    let ws = ProbeWs::new("unblocked");
    assert!(
        !blocks_delete(&ws.missing("ghost.rmeta")),
        "不存在路径判未阻塞（NotFound 不承担归因）"
    );

    let path = ws.file("free.rmeta", "body");
    assert!(!blocks_delete(&path), "无持有者的真实文件判未阻塞");
}

/// 边界（AC-4 / R-4）：只读不扰动自证——判据调用前后文件长度与字节内容不变，
/// 调用返回后立即可 `rename` / 删除（零残留句柄、探针自身零写）。
#[cfg(windows)]
#[test]
fn 判据只读不扰动() {
    let ws = ProbeWs::new("readonly");
    let body = "body-内容-🎉";
    let path = ws.file("keep.rmeta", body);

    let before = std::fs::read(&path).expect("读原文件");
    let probe = blocks_delete(&path);
    let after = std::fs::read(&path).expect("判据后回读");
    assert_eq!(before, after, "探针不改写文件内容");
    assert_eq!(
        std::fs::metadata(&path).expect("取元数据").len(),
        body.len() as u64,
        "探针不改文件长度（无 truncate / create 痕迹）"
    );
    assert!(!probe, "无持有者时判未阻塞");

    let renamed = ws.path().join("renamed.rmeta");
    std::fs::rename(&path, &renamed).expect("判据返回后立即可 rename（零残留句柄）");
    std::fs::remove_file(&renamed).expect("判据返回后立即可删除");
}

/// 边界：非 Windows 臂恒 `false`——POSIX 无强制共享语义，探针不适用。
#[cfg(not(windows))]
#[test]
fn 判据在非windows平台恒false() {
    let ws = ProbeWs::new("posix-false");
    let path = ws.file("lib.rmeta", "body");
    assert!(!blocks_delete(&path), "非 Windows 恒判未阻塞");
    assert!(
        !blocks_delete(&ws.missing("ghost.rmeta")),
        "非 Windows 不存在路径同样为 false"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：lock_holders 点名
// ---------------------------------------------------------------------------

/// 正向（AC-3 / D8）：自锁自证——临时文件由本进程以「读权限 + `share = READ`」
/// 持有 → 点名含本进程（`pid == std::process::id()`），按 `(name, pid)` 去重后
/// 恰 1 条。
#[cfg(windows)]
#[test]
fn 自锁自证点名到本进程() {
    let ws = ProbeWs::new("holders-self");
    let path = ws.file("lib.rmeta", "body");
    let holder = hold_with_share(&path, SHARE_READ_ONLY);

    let holders = lock_holders(std::slice::from_ref(&path));
    drop(holder);

    assert_eq!(
        holders.len(),
        1,
        "按 (name, pid) 去重后恰 1 条: {holders:?}"
    );
    assert_eq!(holders[0].pid, std::process::id(), "点名到持有句柄的本进程");
    assert!(
        !holders[0].name.is_empty(),
        "进程名非空: {:?}",
        holders[0].name
    );
    assert!(
        !holders[0].name.contains('\\') && !holders[0].name.contains('/'),
        "进程名不含路径分隔符: {:?}",
        holders[0].name
    );
}

/// 正向（AC-3）：释放后空清单——关闭上述句柄后再次点名 → 空 vec（「未定位占用者」
/// 由 `attribution_text` 装配），两次调用均不 panic。
#[cfg(windows)]
#[test]
fn 释放句柄后清单为空() {
    let ws = ProbeWs::new("holders-release");
    let path = ws.file("lib.rmeta", "body");
    let holder = hold_with_share(&path, SHARE_READ_ONLY);
    let held = lock_holders(std::slice::from_ref(&path));
    drop(holder);
    let released = lock_holders(std::slice::from_ref(&path));

    assert!(!held.is_empty(), "持有期应点名到本进程");
    assert!(
        released.is_empty(),
        "释放后不再点名（归因降级由 attribution_text 装配）: {released:?}"
    );
}

/// 边界：多路径与部分存在——其一被本进程无 delete 共享持有、其二不存在 → 只
/// 归因存在且阻塞者（不存在路径不 panic、不错误点名）。
#[cfg(windows)]
#[test]
fn 多路径部分存在只点名阻塞者() {
    let ws = ProbeWs::new("holders-mixed");
    let locked = ws.file("lib.rmeta", "body");
    let ghost = ws.missing("ghost.rmeta");
    let holder = hold_with_share(&locked, SHARE_READ_ONLY);

    let holders = lock_holders(&[locked.clone(), ghost]);
    drop(holder);

    assert_eq!(holders.len(), 1, "只点名存在且阻塞者: {holders:?}");
    assert_eq!(holders[0].pid, std::process::id(), "点名对象为本进程");
}

/// 异常（表驱动 3 形态）：空入参 → 空；仅不存在路径 → 空（`NotFound` 静默）；
/// 空入参不触达系统 API（零 panic、零 Err）。
#[test]
fn 空入参与不存在路径皆空清单() {
    assert!(lock_holders(&[]).is_empty(), "空入参直接返回空");

    let ws = ProbeWs::new("holders-empty");
    assert!(
        lock_holders(&[ws.missing("ghost.rmeta")]).is_empty(),
        "不存在路径静默为空（不 Err、不 panic）"
    );
}

/// 异常（AC-4 降级）：非 Windows 平台 `lsof` 缺失 → 空 vec，不 `Err`、不 panic、
/// 不阻断调用方（spec「MUST NOT 阻断主流程」）。
#[cfg(not(windows))]
#[test]
fn 非windows平台lsof缺失静默降级() {
    let ws = ProbeWs::new("lsof-missing");
    let path = ws.file("lib.rmeta", "body");

    // PATH 隔离到空目录：`lsof` 不在 PATH → 静默跳过（不 Err、不 panic）
    let empty_bin = ws.path().join("empty-bin");
    std::fs::create_dir_all(&empty_bin).expect("创建空 PATH 目录");
    let original = std::env::var_os("PATH");
    std::env::set_var("PATH", &empty_bin);

    let holders = lock_holders(std::slice::from_ref(&path));

    match original {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }
    assert!(holders.is_empty(), "lsof 缺失静默为空: {holders:?}");
}

/// 边界：`LockHolder` 两字段面——直构 + `PartialEq` / `clone` / `Debug`（点名
/// 结果的比较与去重基面）。
#[test]
fn lock_holder两字段面() {
    let holder = LockHolder {
        name: "HipsDaemon.exe".to_owned(),
        pid: 1234,
    };
    assert_eq!(holder.name, "HipsDaemon.exe", "进程名可读");
    assert_eq!(holder.pid, 1234, "pid 可读");
    assert_eq!(holder.clone(), holder, "clone 逐字段相等");

    let other = LockHolder {
        name: "HipsDaemon.exe".to_owned(),
        pid: 4321,
    };
    assert_ne!(other, holder, "pid 参与相等判定");
    let debug = format!("{holder:?}");
    assert!(
        debug.contains("HipsDaemon.exe") && debug.contains("1234"),
        "Debug 可读: {debug}"
    );

    // 载荷边界：空名 / 超长名 / 特殊字符 / pid 极值
    for (name, pid) in [
        (String::new(), 0u32),
        ("名".repeat(600), u32::MAX),
        ("a\nb\u{0}c🎉".to_owned(), 1),
    ] {
        let edge = LockHolder {
            name: name.clone(),
            pid,
        };
        assert_eq!(edge.name, name, "进程名逐字保留");
        assert_eq!(edge.pid, pid, "pid 逐值保留");
    }
}

// ---------------------------------------------------------------------------
// 正向 / 异常 / 边界：attribution_text 归因原料
// ---------------------------------------------------------------------------

/// 正向（AC-3）：点名 + 被锁文件——诊断含真实临时路径且该路径被本进程无 delete
/// 共享持有 → `Some`，文本含「占用进程：」+ 本进程 pid 与「被锁文件：」+ 该路径
///（只产原料，不含重试次数 / 处置指引）。
#[cfg(windows)]
#[test]
fn 归因文本含占用进程与被锁文件() {
    let ws = ProbeWs::new("attribution-named");
    let path = ws.file("liba.rlib", "body");
    let holder = hold_with_share(&path, SHARE_READ_ONLY);

    let diagnostics = format!(
        "error: failed to rename archive file: {}\nCaused by: os error 32",
        path.display()
    );
    let text = attribution_text(&diagnostics).expect("路径可解析即产原料");
    drop(holder);

    assert!(text.contains("占用进程："), "首行给出占用者: {text}");
    assert!(
        text.contains(&format!("(pid {})", std::process::id())),
        "点名含本进程 pid: {text}"
    );
    assert!(text.contains("被锁文件："), "次行给出被锁文件: {text}");
    assert!(
        text.contains(&path.to_string_lossy().into_owned()),
        "被锁文件路径逐字: {text}"
    );
    for absent in ["已自动重试", "环境侧处理", "排除"] {
        assert!(
            !text.contains(absent),
            "exec 层不自带业务文案（不得含 {absent}）: {text}"
        );
    }
}

/// 正向（AC-3 / R-3）：空清单降级——诊断含不存在路径（探针判「未阻塞」/ 点名空）
/// → 文本含「未定位占用者（可能为内核过滤驱动（安全软件），无法归因到用户进程）」
/// + 被锁文件清单；MUST NOT 判「无锁」、不返回 `None`。
#[test]
fn 归因文本空清单降级不判无锁() {
    let ws = ProbeWs::new("attribution-degrade");
    let ghost = ws.missing("libb.rlib");
    let diagnostics = format!(
        "error: failed to rename archive file: {}\nCaused by: os error 32",
        ghost.display()
    );

    let text = attribution_text(&diagnostics).expect("路径可解析即产原料（不因点名空返回 None）");

    assert!(
        text.contains("未定位占用者（可能为内核过滤驱动（安全软件），无法归因到用户进程）"),
        "空清单退化为「未定位占用者」说明: {text}"
    );
    assert!(
        text.contains(&ghost.to_string_lossy().into_owned()),
        "被锁文件清单仍承载路径: {text}"
    );
    assert!(!text.contains("无锁"), "MUST NOT 判「无锁」: {text}");
}

/// 异常（表驱动 2 形态）：空串 / 无关键词诊断 → `None`（调用方据此只装配摘要与
/// 指引，不拼空段）。
#[test]
fn 无路径可解析返回none() {
    assert_eq!(attribution_text(""), None, "空诊断无路径可解析");
    assert_eq!(
        attribution_text("error[E0308]: mismatched types\n  --> src/main.rs:3:9"),
        None,
        "无锁关键词诊断不产归因原料"
    );
}

/// 边界（表驱动 2 形态）：占用者可读性——多条阻塞路径（同一持有者）→ 每个
/// `(name, pid)` 去重后不重复；超长诊断（>1000 字符）/ 特殊字符路径 → 文本逐字
/// 承载，不 panic。
#[cfg(windows)]
#[test]
fn 多阻塞路径去重且特殊路径逐字承载() {
    let ws = ProbeWs::new("attribution-multi");
    let first = ws.file("liba.rlib", "a");
    let second = ws.file("libb.rlib", "b");
    let holder_first = hold_with_share(&first, SHARE_READ_ONLY);
    let holder_second = hold_with_share(&second, SHARE_READ_ONLY);

    let diagnostics = format!(
        "error: failed to rename archive file: {}\nerror: failed to write {}",
        first.display(),
        second.display()
    );
    let text = attribution_text(&diagnostics).expect("两条路径均可解析");
    drop(holder_first);
    drop(holder_second);

    let pid_marker = format!("(pid {})", std::process::id());
    assert_eq!(
        text.matches(&pid_marker).count(),
        1,
        "同一持有者 (name, pid) 去重后只出现一次: {text}"
    );
    assert!(
        text.contains(&first.to_string_lossy().into_owned())
            && text.contains(&second.to_string_lossy().into_owned()),
        "两条被锁文件均在案: {text}"
    );

    // 超长诊断 + 特殊字符路径：文本逐字承载，不 panic
    let special = format!(
        "{}\nerror: failed to write /t/a b/构建 🎉.rmeta\n{}",
        "noise ".repeat(200),
        "tail ".repeat(200)
    );
    let long_text = attribution_text(&special).expect("超长诊断仍可解析");
    assert!(
        long_text.contains("/t/a b/构建 🎉.rmeta"),
        "特殊字符路径逐字承载: {long_text}"
    );
    assert!(
        long_text.contains("被锁文件："),
        "超长诊断同样装配被锁文件行: {long_text}"
    );
}
