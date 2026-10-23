//! `prune.rs` 的单元测试（test-design「prune.rs -> prune_test.rs」节）：
//! `PRUNED_DIR_NAMES` 六员封闭集逐字锚定 + 封闭集属性（len / 互异 / 非空 / 不含
//! `openspec` 与 dot 项）+ `is_pruned_dir_name()` 名单命中 / dot 前缀 / 空串 /
//! 普通名 / 形态近邻 / 超长与特殊字符零 panic 直返 bool（纯常量 + 纯函数直测，
//! Mock策略：无——零进程边界依赖）。

use crate::prune::{is_pruned_dir_name, PRUNED_DIR_NAMES};

// ---------------------------------------------------------------------------
// PRUNED_DIR_NAMES：封闭集常量锚
// ---------------------------------------------------------------------------

/// 正向（枚举遍历）：六员逐字锚定——名单与 explore 决议逐字一致（常量组锚：
/// 名单扩展一处修改双点生效的前提面——testexec detect 剪枝与 vcs
/// restore_baseline 排除面两消费点同源）。
#[test]
fn pruned_dir_names六员逐字锚定与explore决议一致() {
    assert_eq!(
        PRUNED_DIR_NAMES,
        [
            "node_modules",
            "dist",
            "build",
            "target",
            "coverage",
            ".nyc_output"
        ],
        "六员逐字锚定（依赖与工具产物树封闭集——名单扩展一处修改双点生效）"
    );
    // 逐员断言（enum 遍历——单员改名即断，不依赖整组比较的定位能力）
    assert_eq!(PRUNED_DIR_NAMES[0], "node_modules");
    assert_eq!(PRUNED_DIR_NAMES[1], "dist");
    assert_eq!(PRUNED_DIR_NAMES[2], "build");
    assert_eq!(PRUNED_DIR_NAMES[3], "target");
    assert_eq!(PRUNED_DIR_NAMES[4], "coverage");
    assert_eq!(PRUNED_DIR_NAMES[5], ".nyc_output");
}

/// 边界（单次）：封闭集属性——`len() == 6`、互异无重复、逐员非空串；名单本体
/// 不含 `openspec` / 纯 dot 项（dot 判定与工件增补驻消费点——单源边界锚）。
#[test]
fn pruned_dir_names封闭集属性_len互异非空且不含openspec() {
    assert_eq!(PRUNED_DIR_NAMES.len(), 6, "封闭集六员（扩员即显式改锚）");

    // 互异无重复（O(n²) 直比——六元小集不引 HashSet）
    for (i, a) in PRUNED_DIR_NAMES.iter().enumerate() {
        for (j, b) in PRUNED_DIR_NAMES.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "名单互异：第 {i} 项 {a:?} 与第 {j} 项 {b:?} 重复");
            }
        }
    }

    // 逐员非空串
    for name in PRUNED_DIR_NAMES.iter() {
        assert!(!name.is_empty(), "名单成员非空串: {name:?}");
    }

    // 名单本体不含 openspec（工件树增补属 vcs 消费点语义，不入名单——单源锚）
    assert!(
        !PRUNED_DIR_NAMES.contains(&"openspec"),
        "openspec 工件树不入名单本体（排除面组装驻 vcs 消费点）"
    );
    // 名单本体不含纯 dot 项（dot 剪枝判定驻 is_pruned_dir_name 前缀语义）
    assert!(
        PRUNED_DIR_NAMES.iter().all(|name| *name != "."),
        "名单本体不含纯 dot 项（dot 前缀判定驻函数语义）"
    );
    // `.nyc_output` 为 dot 前缀形态成员（dot 判定之外显式收录——名单与判定双
    // 通道重叠面逐字锚定）
    assert!(
        PRUNED_DIR_NAMES.contains(&".nyc_output"),
        ".nyc_output 显式收录（CLI excludedDirs 非 dot 成员同集 + dot 成员在案）"
    );
}

// ---------------------------------------------------------------------------
// is_pruned_dir_name()：名单命中 / dot 前缀 / 负面形态
// ---------------------------------------------------------------------------

/// 正向（枚举遍历）：名单逐名命中——六员各一 → `true`。
#[test]
fn is_pruned_dir_name名单六员逐名命中() {
    for name in PRUNED_DIR_NAMES.iter() {
        assert!(
            is_pruned_dir_name(name),
            "名单成员 {name:?} 应命中（依赖与工具产物树剪枝）"
        );
    }
    // 独立于名单引用的字面量再锚一遍（防止常量被误改后逐名测试随之失效）
    for name in [
        "node_modules",
        "dist",
        "build",
        "target",
        "coverage",
        ".nyc_output",
    ] {
        assert!(is_pruned_dir_name(name), "字面量 {name:?} 应命中");
    }
}

/// 正向（单次）：dot 前缀命中——`.git` / `.idea` / `.vscode` / 单字符 `.` →
/// `true`（dot 剪枝语义：任意 dot 起首目录名不依赖名单）。
#[test]
fn is_pruned_dir_name_dot前缀命中() {
    for name in [".git", ".idea", ".vscode", ".", ".github", ".hidden"] {
        assert!(
            is_pruned_dir_name(name),
            "dot 前缀目录名 {name:?} 应命中（dot 剪枝语义）"
        );
    }
}

/// 边界（单次）：空串 → `false`（无前缀无命中）；普通目录名（`src` / `crates` /
/// `openspec`）→ `false`（工件树不进名单本体——openspec 排除归消费点组装）。
#[test]
fn is_pruned_dir_name_空串与普通目录名不命中() {
    assert!(!is_pruned_dir_name(""), "空串无前缀无命中 → false");
    for name in [
        "src", "crates", "openspec", "packages", "changes", "reports",
    ] {
        assert!(
            !is_pruned_dir_name(name),
            "普通目录名 {name:?} 不应命中（工件树 / 源码树不剪枝）"
        );
    }
}

/// 边界（单次）：形态近邻不误伤——`node_module`（少 s）/ `dist-x` /
/// `Build`（大小写折叠）/ 带 `/` 分隔符形态 → `false`（逐字比对零归一；路径
/// 切分归消费点）。
#[test]
fn is_pruned_dir_name_形态近邻不误伤() {
    for name in [
        "node_module",   // 少 s
        "node_modulesx", // 带尾缀
        "dist-x",        // 带连字符尾缀
        "distx",
        "Build", // 大写折叠
        "DIST",
        "Target",
        "COVERAGE",
        "dist/",         // 带分隔符形态（路径切分归消费点）
        "/dist",         // 前导分隔符
        "src/dist",      // 嵌套形态（逐字比对不递归）
        "node_modules/", // 带尾分隔符
        " dist",         // 前导空白
        "dist ",         // 尾随空白
    ] {
        assert!(
            !is_pruned_dir_name(name),
            "形态近邻 {name:?} 不应命中（逐字比对零归一零清洗）"
        );
    }
}

/// 边界（单次）：超长名（>1000 字符）与特殊字符名（emoji / `\n` 字节串）零
/// panic 直返 `bool`（纯函数稳健面——starts_with / contains 零清洗零校验）。
#[test]
fn is_pruned_dir_name_超长与特殊字符零panic直返bool() {
    // 超长普通名 → false（不 panic）
    let long_plain = "a".repeat(1500);
    assert!(
        !is_pruned_dir_name(&long_plain),
        "超长普通名不命中且不 panic"
    );

    // 超长名单近邻名 → false（前缀命中即断的形态：名单命中需逐字全等）
    let long_near = format!("node_modules{}", "x".repeat(1000));
    assert!(
        !is_pruned_dir_name(&long_near),
        "超长近邻名不命中（逐字全等）"
    );

    // 超长 dot 名 → true（dot 前缀语义对超长名同样成立）
    let long_dot = format!(".{}", "b".repeat(1000));
    assert!(is_pruned_dir_name(&long_dot), "超长 dot 名命中（前缀语义）");

    // 特殊字符名 → 依语义直返 bool、零 panic
    assert!(!is_pruned_dir_name("🚀"), "emoji 名不命中且不 panic");
    assert!(is_pruned_dir_name(".🚀"), "dot + emoji 命中（dot 前缀）");
    assert!(!is_pruned_dir_name("突\n变"), "含换行名不命中且不 panic");
    assert!(
        !is_pruned_dir_name("\n"),
        "换行字符自身非 dot 非名单 → false 不 panic"
    );
    assert!(
        !is_pruned_dir_name("line\nbreak"),
        "内嵌换行名不命中且不 panic"
    );
}
