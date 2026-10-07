use crate::globmatch::{match_glob, try_expand_braces};

// ---------------------------------------------------------------------------
// 花括号展开（picomatch `{a,b}` 方言子集）
// ---------------------------------------------------------------------------

/// 正向：交替组展开（单组 / 多组 / 注册表缺省 glob 形态），展开序稳定
/// （前缀序：首组选枝先展开）。
#[test]
fn 花括号展开_交替组与多组() {
    assert_eq!(
        try_expand_braces("*.{ts,tsx}").expect("单组应展开"),
        vec!["*.ts", "*.tsx"]
    );
    assert_eq!(
        try_expand_braces("{a,b}{c,d}").expect("多组应展开"),
        vec!["ac", "ad", "bc", "bd"]
    );
    // 注册表缺省 glob（jest / vitest / vite-plus 档）——8 变体全枚举
    let variants = try_expand_braces("**/*.{test,spec}.{js,ts,jsx,tsx}").expect("缺省 glob 应展开");
    assert_eq!(variants.len(), 8, "两组 2×4 交替");
    assert!(variants.contains(&"**/*.test.tsx".to_owned()));
    assert!(variants.contains(&"**/*.spec.js".to_owned()));
    // node-test 档缺省 glob
    assert_eq!(
        try_expand_braces("**/*.test.{mjs,js,cjs}").expect("node-test 缺省 glob 应展开"),
        vec!["**/*.test.mjs", "**/*.test.js", "**/*.test.cjs"]
    );
}

/// 正向：嵌套组递归（无逗号外层组经内层逗号生效——bash / picomatch 同
/// 语义：`a{b{c,d}}` → abc / abd，非把外层括号按字面保留）。
#[test]
fn 花括号展开_嵌套组递归() {
    assert_eq!(
        try_expand_braces("a{b{c,d}}").expect("嵌套组应递归展开"),
        vec!["abc", "abd"]
    );
    assert_eq!(
        try_expand_braces("{a,b{1,2}}").expect("顶层选枝含嵌套组"),
        vec!["a", "b1", "b2"]
    );
}

/// 边界：无逗号组 / 未闭合花括号按字面保留（picomatch 同口径）。
#[test]
fn 花括号展开_无逗号与未闭合按字面() {
    assert_eq!(
        try_expand_braces("{foo}").expect("无逗号组字面"),
        vec!["{foo}"]
    );
    // 无逗号组字面，其后可展开组照常展开
    assert_eq!(
        try_expand_braces("a{b}c{d,e}").expect("混合应展开"),
        vec!["a{b}cd", "a{b}ce"]
    );
    assert_eq!(
        try_expand_braces("a{b,c").expect("未闭合字面"),
        vec!["a{b,c"]
    );
}

/// 边界：空选枝（`{,a}`）展开为空串与选枝两形态。
#[test]
fn 花括号展开_空选枝() {
    assert_eq!(
        try_expand_braces("x{,a}").expect("空选枝应展开"),
        vec!["x", "xa"]
    );
}

/// 异常：变体数超上限（256）→ 显式 `Err`（枚举面报错原料）。
#[test]
fn 花括号展开_超限err() {
    // 10 组二选一 = 2^10 = 1024 变体 > 256
    let pattern = "{a,b}".repeat(10);
    let error = try_expand_braces(&pattern).expect_err("组合爆炸应 Err");
    assert!(error.contains("花括号展开超限"), "Err 记因，实际: {error}");
}

// ---------------------------------------------------------------------------
// match_glob（CLI matchGlob 同语义 + 花括号方言）
// ---------------------------------------------------------------------------

/// 正向：花括号形态命中 / 不命中、`*` 不跨分隔符、`**` 跨分隔符。
#[test]
fn match_glob_花括号通配语义() {
    assert!(
        match_glob("src/a.test.ts", "**/*.{test,spec}.{ts,tsx}"),
        "花括号变体命中"
    );
    assert!(
        match_glob("src/a.spec.tsx", "**/*.{test,spec}.{ts,tsx}"),
        "第二组变体命中"
    );
    assert!(
        !match_glob("src/a.other.ts", "**/*.{test,spec}.{ts,tsx}"),
        "非交替选枝不命中"
    );
    assert!(
        !match_glob("src/a.test.mts", "**/*.{test,spec}.{ts,tsx}"),
        "扩展不在选枝不命中"
    );
    // `*` 不跨路径分隔符、`**` 跨（require_literal_separator 语义）
    assert!(
        !match_glob("src/a/b.ts", "src/*.{ts,tsx}"),
        "`*` 不跨分隔符"
    );
    assert!(
        match_glob("src/a/b.ts", "src/**/*.{ts,tsx}"),
        "`**` 跨分隔符"
    );
}

/// 边界：花括号形态不享受目录前缀语义（picomatch 口径——含 `{` 即走
/// 全匹配，无通配符变体仅精确等值命中）。
#[test]
fn match_glob_花括号不享受目录前缀() {
    assert!(
        !match_glob("src/app/x.ts", "src/{app,lib}"),
        "花括号无通配符变体不按目录前缀"
    );
    assert!(match_glob("src/app", "src/{app,lib}"), "变体精确等值命中");
}

/// 正向：无通配符模式按目录前缀（相等或 `pattern/` 起手；段边界不误命中）。
#[test]
fn match_glob_无通配符目录前缀() {
    assert!(match_glob("src/app/x.ts", "src/app"), "前缀起手命中");
    assert!(!match_glob("src/appx/x.ts", "src/app"), "段边界不命中");
    assert!(match_glob("src/app", "src/app"), "相等命中");
}

/// 边界：展开超限按不命中收敛（`Pattern::new` 非法模式同口径）。
#[test]
fn match_glob_超限与非法模式不命中() {
    assert!(!match_glob("a", &"{a,b}".repeat(10)), "超限展开不命中");
    assert!(!match_glob("a.ts", "["), "非法字符类不命中");
}
