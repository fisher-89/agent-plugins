//! `scan::owner` 的单元测试：归属单点四形态（唯一最长前缀 / 同长多命中 /
//! 无命中 / `off` 元素仍可归属），`path` 归一三写法，文件级粒度，空前缀与规模
//! 边界。
//!
//! 零 mock：`ArchiElement` 直接构造（纯值，不经解析——AC-2 要求「直接造模型」）；
//! 零 IO（纯字符串前缀匹配）。

use std::collections::BTreeMap;

use crate::dto::{ArchiElement, ElementKind, GateTier, NodeMark};

use super::owner::{Owner, OwnerIndex};

/// 元素构造器（`path` 原文直传，不经归一——归一是被测行为）。
fn element(fqn: &str, gate: GateTier, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate,
        path: paths.iter().map(|path| (*path).to_owned()).collect(),
        summary: None,
        metadata: BTreeMap::new(),
        model_file: ".dev-team/architecture/models/01-test.toml".to_owned(),
        model_line: 1,
        mark: NodeMark::None,
    }
}

/// 单元素命中（`Owner::Element` 的 fqn 断言辅助）。
fn owned_fqn(owner: &Owner) -> &str {
    match owner {
        Owner::Element { fqn, .. } => fqn,
        other => panic!("期望 Element 归属，实际：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 正向：最长前缀唯一命中
// ---------------------------------------------------------------------------

/// 正向（AC-2）：`App`(`./src/`) + `App.Auth`(`./src/auth/`) 两级模型——深层文件
/// 归最深元素，浅层文件归上层元素（取最长而非首个命中）。
#[test]
fn 最长前缀唯一命中() {
    let index = OwnerIndex::build(&[
        element("App", GateTier::Warn, &["./src/"]),
        element("App.Auth", GateTier::Warn, &["./src/auth/"]),
    ]);
    assert_eq!(
        owned_fqn(&index.owner_of("src/auth/token.ts")),
        "App.Auth",
        "深层文件归最长前缀元素"
    );
    assert_eq!(
        owned_fqn(&index.owner_of("src/other.ts")),
        "App",
        "仅被浅层前缀覆盖的文件归上层元素"
    );
}

/// 正向（AC-4）：`gate` 三档逐档透传（定级依据随归属单点出线）。
#[test]
fn gate档位逐档透传() {
    let cases = [
        (
            "App.Strict",
            GateTier::Strict,
            "src/strict/x.ts",
            "src/strict/",
        ),
        ("App.Warn", GateTier::Warn, "src/warn/x.ts", "src/warn/"),
        ("App.Off", GateTier::Off, "src/off/x.ts", "src/off/"),
    ];
    for (fqn, gate, probe, path) in cases {
        let index = OwnerIndex::build(&[element(fqn, gate, &[path])]);
        match index.owner_of(probe) {
            Owner::Element {
                fqn: hit,
                gate: tier,
            } => {
                assert_eq!(hit, fqn, "{probe} 归属 {fqn}");
                assert_eq!(tier, gate, "gate 应逐档透传（定级依据随归属出线）");
            }
            other => panic!("{probe} 期望 Element 归属，实际：{other:?}"),
        }
    }
}

/// 边界（D6）：文件只被 `gate = off` 元素覆盖 → 仍返回 `Element`（不折叠成
/// `None`；「计 unmatched」由调用侧决定）。
#[test]
fn off元素仍可归属() {
    let index = OwnerIndex::build(&[element("App.Legacy", GateTier::Off, &["./src/legacy/"])]);
    match index.owner_of("src/legacy/old.ts") {
        Owner::Element { fqn, gate } => {
            assert_eq!(fqn, "App.Legacy");
            assert_eq!(gate, GateTier::Off, "档位原样返回，不特判");
        }
        other => panic!("off 元素仍应可归属，实际：{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 异常：同长多命中不任选
// ---------------------------------------------------------------------------

/// 异常（AC-2）：两元素同长前缀同时覆盖某文件 → `Ambiguous`，候选 FQN 字典序且
/// 两次调用相等（确定性）。
#[test]
fn 同长多命中不任选() {
    let index = OwnerIndex::build(&[
        element("App.Zeta", GateTier::Warn, &["./src/aa/"]),
        element("App.Alpha", GateTier::Warn, &["src/aa"]),
    ]);
    let first = index.owner_of("src/aa/x.ts");
    let second = index.owner_of("src/aa/x.ts");
    assert_eq!(first, second, "两次归属结果必须相等（不随内部序漂移）");
    match first {
        Owner::Ambiguous(candidates) => {
            assert_eq!(
                candidates,
                vec!["App.Alpha".to_owned(), "App.Zeta".to_owned()],
                "候选 FQN 应字典序"
            );
        }
        other => panic!("同长多命中应 Ambiguous，实际：{other:?}"),
    }
}

/// 异常：三元素同长重叠 → 三候选全进 `Ambiguous`（不截断、不任选）。
#[test]
fn 三元素同长多命中全量候选() {
    let index = OwnerIndex::build(&[
        element("App.C", GateTier::Warn, &["src/aa/"]),
        element("App.B", GateTier::Warn, &["./src/aa/"]),
        element("App.A", GateTier::Warn, &["src\\aa"]),
    ]);
    match index.owner_of("src/aa/x.ts") {
        Owner::Ambiguous(candidates) => {
            assert_eq!(
                candidates,
                vec!["App.A".to_owned(), "App.B".to_owned(), "App.C".to_owned()],
                "三种写法的同长前缀归一后全为候选，且字典序"
            );
        }
        other => panic!("三元素同长重叠应 Ambiguous，实际：{other:?}"),
    }
}

/// 异常：无命中——空索引与「索引只覆盖别的目录」两形态均 `None`。
#[test]
#[allow(non_snake_case)]
fn 无命中返回None() {
    let empty = OwnerIndex::build(&[]);
    assert_eq!(
        empty.owner_of("src/billing/x.ts"),
        Owner::None,
        "空索引恒无主"
    );

    let index = OwnerIndex::build(&[element("App.Auth", GateTier::Warn, &["src/auth/"])]);
    assert_eq!(
        index.owner_of("src/billing/x.ts"),
        Owner::None,
        "前缀不覆盖即无主"
    );
}

// ---------------------------------------------------------------------------
// 边界：path 归一变体
// ---------------------------------------------------------------------------

/// 边界：`path` 三写法（`./src/auth/` / `src\auth` / `src/auth`）归一后等价。
#[test]
fn path归一三写法等价() {
    let forms = ["./src/auth/", "src\\auth", "src/auth"];
    let mut results: Vec<Option<String>> = Vec::new();
    for form in forms {
        let index = OwnerIndex::build(&[element("App.Auth", GateTier::Warn, &[form])]);
        match index.owner_of("src/auth/token.ts") {
            Owner::Element { fqn, .. } => results.push(Some(fqn)),
            Owner::Ambiguous(_) => results.push(Some("<ambiguous>".to_owned())),
            Owner::None => results.push(None),
        }
    }
    assert_eq!(
        results,
        vec![
            Some("App.Auth".to_owned()),
            Some("App.Auth".to_owned()),
            Some("App.Auth".to_owned())
        ],
        "三种写法归一后对同一文件结果相等"
    );
}

/// 边界：文件级 `path` 是精确全等——同目录其他文件不归该元素。
#[test]
fn 文件级path精确匹配() {
    let index = OwnerIndex::build(&[element(
        "App.Auth",
        GateTier::Warn,
        &["./src/auth/index.ts"],
    )]);
    assert_eq!(
        owned_fqn(&index.owner_of("src/auth/index.ts")),
        "App.Auth",
        "文件级 path 全等命中"
    );
    assert_eq!(
        index.owner_of("src/auth/other.ts"),
        Owner::None,
        "同目录其他文件不命中文件级 path"
    );
}

/// 边界：空条目（`""` 与归一后为空的 `"./"`）一律不入选（`build` 只收非空前缀，
/// 不成为兜底归属）；前缀抢占对照取「较短非空前缀 vs 较长前缀」，深层文件恒归最长。
#[test]
fn 根前缀与空条目() {
    for empty in ["", "./"] {
        let index = OwnerIndex::build(&[element("App.Empty", GateTier::Warn, &[empty])]);
        assert_eq!(
            index.owner_of("src/auth/token.ts"),
            Owner::None,
            "空条目 {empty:?} 归一为空前缀后丢弃（不成为兜底归属）"
        );
    }

    let prefix_pair = OwnerIndex::build(&[
        element("App", GateTier::Warn, &["src"]),
        element("App.Auth", GateTier::Warn, &["src/auth/"]),
    ]);
    assert_eq!(
        owned_fqn(&prefix_pair.owner_of("src/billing/x.ts")),
        "App",
        "较短前缀覆盖任意深层文件"
    );
    assert_eq!(
        owned_fqn(&prefix_pair.owner_of("src/auth/token.ts")),
        "App.Auth",
        "前缀不被较短者抢占（最长前缀优先）"
    );
}

/// 边界：空元素列表 → 任何查询恒 `None`（零 panic）。
#[test]
fn 空元素列表恒无主() {
    let index = OwnerIndex::build(&[]);
    for probe in ["", "src/a.ts", "a.ts", "./src/a.ts"] {
        assert_eq!(index.owner_of(probe), Owner::None, "{probe:?} 应无主");
    }
}

// ---------------------------------------------------------------------------
// 边界：规模与字符面
// ---------------------------------------------------------------------------

/// 边界：300 元素（其中 100 条同长重叠）→ 命中项恒为最长，同长重叠为
/// `Ambiguous`；构建一次后多次查询结果稳定。
#[test]
fn 三百元素规模与重叠前缀() {
    let mut elements: Vec<ArchiElement> = Vec::new();
    for index in 0..100 {
        elements.push(element(
            &format!("App.Overlap{index:03}"),
            GateTier::Warn,
            &["src/overlap/"],
        ));
    }
    for index in 0..200 {
        elements.push(element(
            &format!("App.Distinct{index:03}"),
            GateTier::Warn,
            &[format!("src/distinct{index:03}/").as_str()],
        ));
    }
    let index = OwnerIndex::build(&elements);

    // 重叠面对面：恒 Ambiguous，两次查询相等。
    let first = index.owner_of("src/overlap/x.ts");
    let second = index.owner_of("src/overlap/x.ts");
    assert_eq!(first, second, "规模下结果仍确定性");
    match &first {
        Owner::Ambiguous(candidates) => {
            assert_eq!(candidates.len(), 100, "100 条同长重叠全为候选");
        }
        other => panic!("重叠前缀应 Ambiguous，实际：{other:?}"),
    }

    // 五个互不重叠的探针：恒最长且唯一。
    for probe_index in [0usize, 1, 42, 137, 199] {
        let probe = format!("src/distinct{probe_index:03}/x.ts");
        assert_eq!(
            owned_fqn(&index.owner_of(&probe)),
            format!("App.Distinct{probe_index:03}"),
            "{probe} 命中最长唯一前缀"
        );
    }
}

/// 边界：不做大小写归一（`src/Auth/token.ts` 对 `src/auth/` 无主）；带前后空格的
/// 路径不 panic 且结果可判。
#[test]
fn 大小写敏感与空格路径() {
    let index = OwnerIndex::build(&[element("App.Auth", GateTier::Warn, &["src/auth/"])]);
    assert_eq!(
        index.owner_of("src/Auth/token.ts"),
        Owner::None,
        "不做大小写归一（词面精确）"
    );
    let spaced = index.owner_of("  src/auth/token.ts  ");
    assert_eq!(spaced, Owner::None, "前后空格不参与归一，结果可判");
}

/// 边界：非 ASCII 路径可归属；>1000 chars 路径不 panic 且 `None`。
#[test]
#[allow(non_snake_case)]
fn 非ASCII与超长路径() {
    let index = OwnerIndex::build(&[element("App.Auth", GateTier::Warn, &["src/认证/"])]);
    assert_eq!(
        owned_fqn(&index.owner_of("src/认证/token.ts")),
        "App.Auth",
        "非 ASCII 前缀可归属"
    );

    let long_unowned = format!("other/{}.ts", "a".repeat(1200));
    assert_eq!(
        index.owner_of(&long_unowned),
        Owner::None,
        "超长路径不 panic 且无主"
    );
    let long_owned = format!("src/认证/{}.ts", "a".repeat(1200));
    assert_eq!(
        owned_fqn(&index.owner_of(&long_owned)),
        "App.Auth",
        "超长路径仍按前缀归属（不做长度上限）"
    );
}
