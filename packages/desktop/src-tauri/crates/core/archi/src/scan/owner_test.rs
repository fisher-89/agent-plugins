//! `scan::owner` 的单元测试：归属单点四形态（唯一最长前缀 / 同长多命中 /
//! 无命中 / `off` 元素仍可归属），`path` 归一三写法，文件级粒度，空前缀与规模
//! 边界。
//!
//! 零 mock：`ArchiElement` 直接构造（纯值，不经解析——AC-2 要求「直接造模型」）；
//! 零 IO（纯字符串前缀匹配）。

use std::collections::BTreeMap;

use crate::dto::{ApiStatus, ArchiElement, ElementKind, GateTier, NodeMark};

use super::owner::{Owner, OwnerIndex};

/// 元素构造器（`path` 原文直传，不经归一——归一是被测行为）。
fn element(fqn: &str, gate: GateTier, paths: &[&str]) -> ArchiElement {
    ArchiElement {
        fqn: fqn.to_owned(),
        kind: ElementKind::Module,
        gate,
        api: ApiStatus::Declared,
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

// ===========================================================================
// `.i.<ext>` 生成物剥离（`module-api-contract-format` / D3）
//
// 剥离判据 = 末段形如 `<stem>.i.<ext>`（`ext ∈ {rs, ts, tsx, js, jsx}`）时把该段替换为
// `<stem>.<ext>`；`normalize` / `prefix_matches` / `build` 语义逐字零变化，非
// `.i.<ext>` 路径的归属结果与改动前完全相同。
// ===========================================================================

/// 正向：剥后形态与原模块文件同名（`views/architecture.tsx` ← `architecture.tsx.i.ts`
/// 走 `<stem>.<ext>` 规则时不等，故取真正的等价形态）——单文件元素的生成物
/// `a.tsx.i.ts` 剥成 `a.tsx.ts`，与 `a.tsx` 不等；**目录内**的生成物由目录前缀命中。
#[test]
fn 生成物剥离四种形态各归其元素() {
    let index = OwnerIndex::build(&[
        element(
            "App.Frontend.Views.Architecture",
            GateTier::Off,
            &["views/architecture/"],
        ),
        element(
            "App.Frontend.Views.Welcome",
            GateTier::Off,
            &["views/welcome-view/"],
        ),
        element(
            "App.Backend.Core.Archi",
            GateTier::Off,
            &["crates/core/archi/"],
        ),
        element(
            "App.Backend.Core.Archi.Scan",
            GateTier::Off,
            &["crates/core/archi/src/scan/"],
        ),
    ]);

    // 四种「目录形态」生成物：剥后仍落在目录内，由 `prefix + "/"` 判据命中。
    let cases = [
        (
            "views/architecture/architecture.i.ts",
            "App.Frontend.Views.Architecture",
        ),
        (
            "views/welcome-view/welcome-view.i.ts",
            "App.Frontend.Views.Welcome",
        ),
        ("crates/core/archi/archi.i.rs", "App.Backend.Core.Archi"),
        (
            "crates/core/archi/src/scan/scan.i.rs",
            "App.Backend.Core.Archi.Scan",
        ),
    ];
    for (probe, expected) in cases {
        assert_eq!(
            owned_fqn(&index.owner_of(probe)),
            expected,
            "{probe} 剥后缀后由目录前缀命中"
        );
    }

    // 非 `.i` 路径的归属与改动前逐字相同
    assert_eq!(
        owned_fqn(&index.owner_of("crates/core/archi/src/dto.rs")),
        "App.Backend.Core.Archi",
        "非 `.i` 路径归属零变化"
    );
}

/// 正向：剥离的唯一实质效果是「根同级生成物」——元素根取**带扩展名的单文件形态**
/// 时，`<root>.i.<ext>` 剥成 `<root>.<ext>` 后与元素根全等（`rel == prefix` 分支）；
/// 目录形态元素根（带尾 `/`）的根同级生成物仍不命中（目录前缀要求 `/` 分隔）。
#[test]
fn 生成物剥离使单文件元素根同级生成物归元素() {
    let single = OwnerIndex::build(&[element(
        "App.View.Welcome",
        GateTier::Warn,
        &["views/welcome.ts"],
    )]);
    // 剥成 `views/welcome.ts` 与元素根全等（`rel == prefix` 分支）→ 命中
    assert_eq!(
        owned_fqn(&single.owner_of("views/welcome.i.ts")),
        "App.View.Welcome",
        "单文件元素根（views/welcome.ts）的根同级生成物剥后全等"
    );
    // 其余扩展名剥后是 `views/welcome.rs` / `.tsx` / `.js` / `.jsx`，与元素根不等 → 无主
    for extension in ["rs", "tsx", "js", "jsx"] {
        assert_eq!(
            single.owner_of(&format!("views/welcome.i.{extension}")),
            Owner::None,
            "views/welcome.i.{extension} 剥后与 views/welcome.ts 不等 → 如实无主"
        );
    }

    let directory = OwnerIndex::build(&[element("App.Core.Dir", GateTier::Warn, &["src/dir/"])]);
    assert_eq!(
        directory.owner_of("src/dir.i.ts"),
        Owner::None,
        "目录形态元素根的根同级生成物剥后不在目录内 → 如实无主"
    );
    assert_eq!(
        owned_fqn(&directory.owner_of("src/dir/dir.i.ts")),
        "App.Core.Dir",
        "目录形态元素根的目录内生成物由目录前缀命中"
    );
}

/// 边界（D3 反例）：白名单外的后缀（`.i.py` / `.d.rs` / 无扩展名 `.i` / 大写标记
/// `.I.rs`）→ 剥离不生效，归属按原路径判定，与改动前逐值相等。
#[test]
fn 生成物白名单外后缀不剥离() {
    let index = OwnerIndex::build(&[element("App.Core.Mod", GateTier::Warn, &["src/mod/"])]);
    // 目录内：剥与不剥都由目录前缀命中（这一面无法区分剥离是否发生）
    for probe in ["src/mod/mod.i.py", "src/mod/mod.d.rs", "src/mod/mod.i"] {
        assert_eq!(
            owned_fqn(&index.owner_of(probe)),
            "App.Core.Mod",
            "{probe} 落在元素目录内 → 剥离不生效也照常归属"
        );
    }
    // 目录外：剥离生效才会命中，故白名单外后缀必须落 None
    let file_root = OwnerIndex::build(&[element("App.MyMod", GateTier::Warn, &["src/my.mod.rs"])]);
    for probe in [
        "src/my.mod.i.py",
        "src/my.mod.d.rs",
        "src/my.mod.i",
        "src/my.mod.I.rs",
    ] {
        assert_eq!(
            file_root.owner_of(probe),
            Owner::None,
            "{probe} 不在五扩展名 / 标记白名单内 → 无主（零误剥）"
        );
    }
    // 对照：白名单内同前缀形态归元素
    assert_eq!(
        owned_fqn(&file_root.owner_of("src/my.mod.i.rs")),
        "App.MyMod",
        "白名单内后缀正常剥离"
    );
}

/// 边界（剥的是整组 `.i.<ext>`，不是只去 `.i`）：`a.i.ts` → `a.ts`（与元素根全等）；
/// `<root>` 自身含点时只剥最后一组（`my.mod.i.rs` → `my.mod.rs`）。
#[test]
fn 生成物剥离只去末段标记() {
    let index = OwnerIndex::build(&[
        element("App.A", GateTier::Warn, &["src/a.ts"]),
        element("App.MyMod", GateTier::Warn, &["src/my.mod.rs"]),
    ]);
    assert_eq!(
        owned_fqn(&index.owner_of("src/a.i.ts")),
        "App.A",
        "`a.i.ts` 剥成 `a.ts`（末段整组替换，不是只去 `.i`）"
    );
    assert_eq!(
        owned_fqn(&index.owner_of("src/my.mod.i.rs")),
        "App.MyMod",
        "`<root>` 自身含点时只剥最后一组 `.i.<ext>`"
    );
    // 错判据对照：只去 `.i` 会得到 `src/a.ts.ts` / `src/my.mod.rs.rs`（皆不命中原文件）
    let wrong_stem = OwnerIndex::build(&[element("App.A", GateTier::Warn, &["src/a.ts.ts"])]);
    assert_eq!(
        wrong_stem.owner_of("src/a.i.ts"),
        Owner::None,
        "若按「只去 `.i`」实现，剥后形态与真实元素根不等 → 可判为错"
    );
}

/// 边界：末段为空 stem（`.i.rs`，其 `rsplit_once('.')` 的 module 为空）/ 以 `/` 结尾 →
/// 归属落封闭三值内且零 panic；反斜杠分隔与 `./` 前缀的生成物仍剥。
#[test]
fn 生成物边界形态不panic() {
    let index = OwnerIndex::build(&[element("App.MyMod", GateTier::Warn, &["src/my.mod.rs"])]);
    for probe in [".i.rs", "src/my.mod.i.rs/"] {
        let owner = index.owner_of(probe);
        assert!(
            matches!(
                owner,
                Owner::Element { .. } | Owner::Ambiguous(_) | Owner::None
            ),
            "{probe} 的归属落在封闭三值内：{owner:?}"
        );
    }
    assert_eq!(
        owned_fqn(&index.owner_of("src\\my.mod.i.rs")),
        "App.MyMod",
        "反斜杠分隔的生成物路径同样剥后缀"
    );
    assert_eq!(
        owned_fqn(&index.owner_of("./src/my.mod.i.rs")),
        "App.MyMod",
        "`./` 前缀由 normalize 剥离（与剥后缀互不干扰）"
    );
}

/// 边界：剥离不改变歧义语义——同长多命中的生成物仍 `Ambiguous`（候选全量、字典序）。
#[test]
fn 生成物形态下同长多命中仍歧义() {
    let overlap = OwnerIndex::build(&[
        element("App.C", GateTier::Warn, &["src/x.ts"]),
        element("App.D", GateTier::Warn, &["src/x.ts"]),
    ]);
    match overlap.owner_of("src/x.i.ts") {
        Owner::Ambiguous(candidates) => {
            assert_eq!(candidates.len(), 2, "剥后同长多命中仍为全量候选");
            let mut sorted = candidates.clone();
            sorted.sort();
            assert_eq!(candidates, sorted, "候选字典序");
        }
        other => panic!("同长多命中应 Ambiguous，实际：{other:?}"),
    }
}

/// 边界（AC-16）：非生成物路径的归属结果与改动前逐值相等（新增剥离只对 `.i.<ext>`
/// 生效）——三条既有点位逐条回归 + 文件名含 `.i` 的非生成物。
#[test]
fn 非生成物路径归属零变化() {
    let index = OwnerIndex::build(&[
        element(
            "App.Frontend.Views.Architecture",
            GateTier::Off,
            &["views/architecture.tsx"],
        ),
        element(
            "App.Frontend.Views.Welcome",
            GateTier::Off,
            &["views/welcome-view/"],
        ),
        element(
            "App.Backend.Core.Archi",
            GateTier::Off,
            &["crates/core/archi/"],
        ),
    ]);
    assert_eq!(
        owned_fqn(&index.owner_of("views/architecture.tsx")),
        "App.Frontend.Views.Architecture",
        "单文件元素的原文件归属不变"
    );
    assert_eq!(
        owned_fqn(&index.owner_of("views/welcome-view/index.ts")),
        "App.Frontend.Views.Welcome",
        "目录元素的普通源文件归属不变"
    );
    assert_eq!(
        owned_fqn(&index.owner_of("crates/core/archi/src/scan/rust.rs")),
        "App.Backend.Core.Archi",
        "目录内的普通源文件归属不变（无第二套命名）"
    );
    for probe in ["views/welcome-view/a.i.txt", "views/welcome-view/a.i"] {
        assert_eq!(
            owned_fqn(&index.owner_of(probe)),
            "App.Frontend.Views.Welcome",
            "{probe} 不是生成物 → 由目录前缀照常归属"
        );
    }
}

/// 边界（D3 单源）：两常量各只定义一处（`include_str!` 计数自证）；五扩展名逐值可剥
/// （缺 `js` / `jsx` 会落 `Owner::None`，即 D3 反例）。
#[test]
fn 生成物常量单源与五扩展名齐备() {
    let source = include_str!("owner.rs");
    assert_eq!(
        source.matches("const GENERATED_MARKER").count(),
        1,
        "标记常量恰定义一处"
    );
    assert_eq!(
        source.matches("const GENERATED_EXTENSIONS").count(),
        1,
        "扩展名常量恰定义一处"
    );
    assert!(
        source.contains("const GENERATED_MARKER: &str = \"i\";"),
        "标记值为 `i`"
    );
    assert!(
        source.contains("[&str; 5]"),
        "扩展名常量为五元数组（长度即判据）"
    );

    // 行为面：五扩展名各剥后命中（以文件名形态的元素根为基准）
    let index = OwnerIndex::build(&[element("App.Core.Mod", GateTier::Warn, &["src/mod.ts"])]);
    let mut hit = Vec::new();
    for extension in ["rs", "ts", "tsx", "js", "jsx"] {
        // 每个扩展名用各自可命中的元素根（`<stem>.<ext>` 与元素根全等）
        let path = format!("src/mod.{extension}");
        let one = OwnerIndex::build(&[element("App.Core.Mod", GateTier::Warn, &[path.as_str()])]);
        if matches!(
            one.owner_of(&format!("src/mod.i.{extension}")),
            Owner::Element { .. }
        ) {
            hit.push(extension);
        }
    }
    assert_eq!(
        hit,
        vec!["rs", "ts", "tsx", "js", "jsx"],
        "五扩展名逐值可剥（长度 5、集合封闭）"
    );
    // 缺 js / jsx 的反例：这两类生成物若不被剥，元素根同级形态必落 None
    let js_only = OwnerIndex::build(&[element("App.Core.Js", GateTier::Warn, &["src/js.js"])]);
    assert_eq!(
        owned_fqn(&js_only.owner_of("src/js.i.js")),
        "App.Core.Js",
        "`.i.js` 被剥（D3：漏 js 会让该元素生成物落 None）"
    );
    let _ = index;
}
