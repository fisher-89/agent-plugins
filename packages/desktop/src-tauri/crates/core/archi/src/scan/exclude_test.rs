//! `scan::exclude` 的单元测试：默认七条约定展开后的逐条命中、配置追加
//! （非替换）、非法条目的显式 `Err`、命中下标的最小值确定性，以及大小写 /
//! 分隔符形态的判定面。
//!
//! 零 mock：真实 `globset` 编译与匹配（排除语义即被测对象）；零 IO（夹具全是
//! 路径字面量）。

use globset::Glob;

use super::exclude::{ExcludeSet, DEFAULT_EXCLUDE_RULES};

/// 空配置集（默认规则集的基线夹具）。
fn default_set() -> ExcludeSet {
    ExcludeSet::build(&[]).expect("默认排除矩阵必须可编译")
}

/// 字符串切片 → 配置条目向量（`build` 的入参形态）。
fn entries(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

// ---------------------------------------------------------------------------
// 正向：默认七条约定逐条命中
// ---------------------------------------------------------------------------

/// 正向（AC-1）：七条约定各取一条代表路径 → 恒命中，且七条命中的规则原文互不
/// 相同（证明是七条不同约定而非同一条兜底）。
#[test]
fn 默认七条约定逐条命中且规则互异() {
    let set = default_set();
    let probes = [
        "src/a.test.ts",
        "src/a.spec.tsx",
        "src/__tests__/a.ts",
        "crates/x/src/a_test.rs",
        "tests/a.ts",
        "fixtures/a.ts",
        "__mocks__/a.ts",
    ];
    let mut rules: Vec<String> = Vec::new();
    for probe in probes {
        let rule = set
            .matched(probe)
            .unwrap_or_else(|| panic!("默认排除约定应命中 {probe}"));
        rules.push(rule);
    }
    assert_eq!(rules.len(), 7, "七条探针各一条命中记录");

    // 两条 `*.test.*` / `*.spec.*` 命中不同扩展名形态，故规则原文全量去重后
    // 至少覆盖 6 类（7 条探针中 `*.test.ts` 与 `*.spec.tsx` 分属两条约定；
    // `__tests__` / `_test.rs` / `tests` / `fixtures` / `__mocks__` 各一条）。
    let mut distinct = rules.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        rules.len(),
        "七条探针的命中规则应互不相同：{rules:?}"
    );
}

/// 边界（D8）：默认规则常量是**展开后的具体 glob**——条数与源码一致、每条含
/// 通配符、逐条可编译、且不含 `globset` 不支持的花括号交替方言。
#[test]
fn 默认规则常量为展开后的具体glob() {
    assert_eq!(
        DEFAULT_EXCLUDE_RULES.len(),
        13,
        "七条约定展开为 4 + 4 + 1 + 1 + 1 + 1 + 1 = 13 条具体 glob"
    );
    for rule in DEFAULT_EXCLUDE_RULES {
        assert!(
            rule.contains('*') || rule.contains('?') || rule.contains('['),
            "默认规则应为 glob 形态：{rule}"
        );
        assert!(
            !rule.contains('{') && !rule.contains('}'),
            "globset 无花括号交替方言，默认规则 MUST NOT 使用：{rule}"
        );
        Glob::new(rule).unwrap_or_else(|error| panic!("默认规则 {rule} 不可编译：{error}"));
    }
}

/// 边界：`*.test.{ts,tsx,js,jsx}` 与 `*.spec.{ts,tsx,js,jsx}` 八扩展名各自命中
/// （防花括号展开漏项）。
#[test]
fn 扩展名变体全覆盖() {
    let set = default_set();
    for extension in ["ts", "tsx", "js", "jsx"] {
        let test_probe = format!("src/auth/a.test.{extension}");
        let spec_probe = format!("src/auth/a.spec.{extension}");
        assert_eq!(
            set.matched(&test_probe),
            Some(format!("**/*.test.{extension}")),
            "{test_probe} 应命中对应扩展名的 test 规则"
        );
        assert_eq!(
            set.matched(&spec_probe),
            Some(format!("**/*.spec.{extension}")),
            "{spec_probe} 应命中对应扩展名的 spec 规则"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向：未命中
// ---------------------------------------------------------------------------

/// 正向：普通源文件（无配置追加）恒不命中。
#[test]
fn 普通源文件不命中() {
    let set = default_set();
    for probe in ["src/auth/token.ts", "src/auth/token.tsx", "src/legacy/x.rs"] {
        assert_eq!(set.matched(probe), None, "{probe} 不应命中默认排除规则");
    }
}

// ---------------------------------------------------------------------------
// 正向 / 边界：配置追加不替换
// ---------------------------------------------------------------------------

/// 正向（AC-1）：`build(&["src/legacy/**"])` → 配置条目命中的 `rule` 是配置原文，
/// 同时默认规则的命中面不变（追加，非替换）。
#[test]
fn 配置追加不替换默认约定() {
    let set = ExcludeSet::build(&entries(&["src/legacy/**"])).expect("合法配置条目应可编译");
    assert_eq!(
        set.matched("src/legacy/old.ts"),
        Some("src/legacy/**".to_owned()),
        "配置目录内的文件命中配置规则原文"
    );
    assert_eq!(
        set.matched("src/a.test.ts"),
        Some("**/*.test.ts".to_owned()),
        "默认规则在配置追加后仍逐条生效"
    );
}

/// 边界：空配置与默认集逐条行为相同（7 命中 + 未命中集合相等）。
#[test]
fn 空配置与默认集等价() {
    let implicit = default_set();
    let explicit = ExcludeSet::build(&entries(&[])).expect("空配置合法");
    let probes = [
        "src/a.test.ts",
        "src/a.spec.tsx",
        "src/__tests__/a.ts",
        "crates/x/src/a_test.rs",
        "tests/a.ts",
        "fixtures/a.ts",
        "__mocks__/a.ts",
        "src/auth/token.ts",
        "src/auth/token.tsx",
        "src/legacy/x.rs",
    ];
    for probe in probes {
        assert_eq!(
            implicit.matched(probe),
            explicit.matched(probe),
            "{probe} 在空配置与默认集下结果应相同"
        );
    }
}

/// 边界：一条路径同时命中默认规则与配置规则 → 恒取**下标最小**那条（默认规则
/// 在前），两次调用结果相等（不随内部序漂移）。
#[test]
fn 最小下标优先且两次结果相等() {
    let set = ExcludeSet::build(&entries(&["**/*.test.ts"])).expect("同形配置条目合法");
    let first = set.matched("src/a.test.ts");
    let second = set.matched("src/a.test.ts");
    assert_eq!(first, second, "同一输入两次命中结果必须相等（确定性）");
    assert_eq!(
        first,
        Some("**/*.test.ts".to_owned()),
        "默认规则下标在前，命中值应为首个同名规则"
    );
}

// ---------------------------------------------------------------------------
// 异常：非法配置条目
// ---------------------------------------------------------------------------

/// 异常：`Glob::new` 失败的配置条目 → `Err`，文案含该条目原文。
#[test]
fn 配置非法glob显式报错() {
    let error = ExcludeSet::build(&entries(&["src/[legacy"]))
        .err()
        .expect("非法 glob 必须 Err");
    assert!(
        error.contains("src/[legacy"),
        "错误文案应含非法条目原文：{error}"
    );
}

/// 异常：目录形态 / 空白条目（无通配符）→ 全 `Err`（目录须写 `src/legacy/**`）。
#[test]
fn 配置无通配符条目显式报错() {
    for entry in ["src/legacy", "", "src/"] {
        let error = ExcludeSet::build(&entries(&[entry]))
            .err()
            .unwrap_or_else(|| panic!("无通配符条目 {entry:?} 必须 Err"));
        assert!(!error.is_empty(), "错误文案非空");
    }
}

// ---------------------------------------------------------------------------
// 边界：超长 / 特殊字符 / 超大配置
// ---------------------------------------------------------------------------

/// 边界：超长条目（>1000 chars）与含 emoji 的条目——只断言「不 panic + 返回型
/// 可判」（`Ok` / `Err` 皆合法，取决于 glob 语法）。
#[test]
fn 超长与特殊字符条目不panic() {
    let long = format!("{}*.ts", "a".repeat(1200));
    let outcome = ExcludeSet::build(&entries(&[long.as_str()]));
    match outcome {
        Ok(set) => assert!(
            set.matched("src/a.ts")
                .as_deref()
                .is_none_or(|rule| !rule.is_empty()),
            "超长条目若编译成功，命中面仍是合法规则原文"
        ),
        Err(error) => assert!(!error.is_empty(), "超长条目失败面文案非空"),
    }

    let error = ExcludeSet::build(&entries(&["src/🎯/[x"]))
        .err()
        .expect("含 emoji 的非法条目必须 Err");
    assert!(!error.is_empty(), "非法条目文案非空");
}

/// 边界：100 条合法配置 glob → `Ok`；建一次后在 1000 次 `matched()` 中结果稳定
/// （编译复用，不是每次重编译）。
#[test]
fn 超大配置列表并批量复用() {
    let extra: Vec<String> = (0..100)
        .map(|index| format!("src/generated{index}/**"))
        .collect();
    let set = ExcludeSet::build(&extra).expect("100 条合法 glob 应可编译");
    let expected = Some("src/generated42/**".to_owned());
    for _ in 0..1000 {
        assert_eq!(
            set.matched("src/generated42/x.ts"),
            expected,
            "同一编译产物在批量查询中结果稳定"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：路径形态与大小写
// ---------------------------------------------------------------------------

/// 边界：`./` 前缀与绝对形态的路径与正斜杠形态同结果（设计未归一者以「不 panic +
/// 结果可判」表达：反斜杠形态只断言可调用）。
#[test]
fn 路径形态归一() {
    let set = default_set();
    let canonical = set.matched("src/a.test.ts");
    assert_eq!(canonical, Some("**/*.test.ts".to_owned()), "基线形态");
    assert_eq!(
        set.matched("./src/a.test.ts"),
        canonical,
        "剥 `./` 前缀后结果相同"
    );
    assert_eq!(
        set.matched("/src/a.test.ts"),
        canonical,
        "绝对形态与仓相对形态同结果"
    );
    // 反斜杠形态：设计未强制归一，只断言不 panic 且返回型可判。
    let windows_form = set.matched("src\\a.test.ts");
    assert!(
        windows_form.as_deref().is_none_or(|rule| !rule.is_empty()),
        "反斜杠形态必须可判（不 panic），命中的规则原文非空"
    );
}

/// 边界：不做大小写归一——`src/A.Test.TS` 恒 `None`。
#[test]
fn 大小写敏感() {
    let set = default_set();
    assert_eq!(
        set.matched("src/A.Test.TS"),
        None,
        "词面精确匹配，无大小写归一"
    );
}
