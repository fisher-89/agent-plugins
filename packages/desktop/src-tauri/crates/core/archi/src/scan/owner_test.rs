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
// `.i.<ext>` 生成物剥离（`module-api-contract-format` / D3 + 并集判据）
//
// 生成物有两种落点形态且互不相交：① 元素根同级（`<root>.i.rs`，需**整组剥离**
// `archi.i.rs → archi`）、② 元素目录内（`<dir>/<name>.i.<ext>`，需**替换扩展名**
// `archi.i.rs → archi.rs`）。`strip_candidates` 对白名单内的末段返回这两个候选，
// `owner_of` 对每个候选各跑一遍最长前缀、合并去重后按前缀长度定档（**禁止候选
// 短路**）。`normalize` / `prefix_matches` / `build` 语义逐字零变化——非
// `.i.<ext>` 路径的归属结果与改动前完全相同。
// ===========================================================================

/// 正向：剥后形态与原模块文件同名（`views/architecture.tsx` ← `architecture.tsx.i.ts`
/// 走 `<stem>.<ext>` 规则时不等，故取真正的等价形态）——目录形态元素根的**目录内**
/// 生成物由目录前缀命中；根同级生成物由整组剥离候选（`<root>.i.<ext>` → `<root>`）
/// 命中（见下一用例）。
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

/// 正向（AC-7 / 翻转臂）：根同级生成物的两种元素根形态——单文件形（`<root>.<ext>`）
/// 由替换扩展名候选命中、目录形（`<root>/`）由整组剥离候选命中。目录形元素的
/// `src/dir.i.ts` 修复前落 `None`，并集判据下归元素。
#[test]
fn 生成物根同级形态各归其元素() {
    let single = OwnerIndex::build(&[element(
        "App.View.Welcome",
        GateTier::Warn,
        &["views/welcome.ts"],
    )]);
    // 替换扩展名候选 `views/welcome.ts` 与元素根全等（`rel == prefix` 分支）→ 命中
    assert_eq!(
        owned_fqn(&single.owner_of("views/welcome.i.ts")),
        "App.View.Welcome",
        "单文件元素根（views/welcome.ts）的根同级生成物由替换候选命中"
    );
    // 其余扩展名：两候选为 `views/welcome` 与 `views/welcome.{ext}`，皆与元素根不等 → 无主
    for extension in ["rs", "tsx", "js", "jsx"] {
        assert_eq!(
            single.owner_of(&format!("views/welcome.i.{extension}")),
            Owner::None,
            "views/welcome.i.{extension} 两候选均不命中 views/welcome.ts → 如实无主"
        );
    }

    // 目录形元素根：整组剥离候选 `src/dir` 落在目录前缀内 → 命中（修复前为 None）
    let directory = OwnerIndex::build(&[element("App.Core.Dir", GateTier::Warn, &["src/dir/"])]);
    assert_eq!(
        owned_fqn(&directory.owner_of("src/dir.i.ts")),
        "App.Core.Dir",
        "目录形态元素根的根同级生成物由整组剥离候选命中（并集判据翻转臂）"
    );
    assert_eq!(
        owned_fqn(&directory.owner_of("src/dir/dir.i.ts")),
        "App.Core.Dir",
        "目录形态元素根的目录内生成物由目录前缀命中"
    );
}

/// 正向（AC-7 / D1 并集判据）：四条根同级实形态——元素目录旁边的 `<末段>.i.<ext>`
/// 全部归该元素（整组剥离候选生效）。
#[test]
fn 生成物根同级四实形态归元素() {
    let cases = [
        (
            "App.Frontend.Views.Architecture",
            "./views/architecture/",
            "views/architecture/architecture.i.tsx",
        ),
        ("App.Frontend.Lib", "./src/lib/", "src/lib/lib.i.ts"),
        ("App.Backend.Core.X", "./core/x/", "core/x/x.i.rs"),
        (
            "App.Backend.Infra.ArchiRuntime",
            "./crates/infra/archi/",
            "crates/infra/archi.i.rs",
        ),
    ];
    for (fqn, path, probe) in cases {
        let index = OwnerIndex::build(&[element(fqn, GateTier::Warn, &[path])]);
        assert_eq!(
            owned_fqn(&index.owner_of(probe)),
            fqn,
            "{probe} 的整组剥离候选落在 {path} 前缀内 → 归 {fqn}"
        );
    }
}

/// 异常（AC-7 / R-1）：并集定档下更具体的单文件根胜出——若实现成「首条候选命中
/// 即短路」，整组候选 `src/dir` 会先给出 `App.Core.Dir`，本用例必红。
#[test]
fn 并集定档使更具体的单文件根胜出() {
    let index = OwnerIndex::build(&[
        element("App.Core.Dir", GateTier::Warn, &["./src/dir/"]),
        element("App.Core.DirFile", GateTier::Warn, &["./src/dir.ts"]),
    ]);
    assert_eq!(
        owned_fqn(&index.owner_of("src/dir.i.ts")),
        "App.Core.DirFile",
        "替换扩展名候选 `src/dir.ts` 更长 → 归更具体的单文件根（禁止候选短路）"
    );
}

/// 异常（AC-7 / 既有语义保持）：两候选合并后同长仍歧义——三个元素分别被两条候选
/// 命中（整组候选命中 1 个、替换候选命中 2 个），合并后取最长档仍为多命中，
/// `Ambiguous` 全量字典序（不因合并且退化为单值）。
#[test]
fn 生成物并集合并后同长仍歧义() {
    let index = OwnerIndex::build(&[
        element("App.A", GateTier::Warn, &["src/x/"]),
        element("App.B", GateTier::Warn, &["src/x.ts"]),
        element("App.C", GateTier::Warn, &["src/x.ts"]),
    ]);
    match index.owner_of("src/x.i.ts") {
        Owner::Ambiguous(candidates) => {
            assert_eq!(
                candidates,
                vec!["App.B".to_owned(), "App.C".to_owned()],
                "替换候选（`src/x.ts`，更长）的同长多命中全量保留且字典序"
            );
        }
        other => panic!("并集合并后同长应 Ambiguous，实际：{other:?}"),
    }
}

/// 正向（AC-7 / 两候选来源独立可达）：仅目录根时整组候选生效；仅文件根时替换
/// 候选生效——两条候选路径各被一次独立证明。
#[test]
fn 两候选来源各自独立可达() {
    let directory_only =
        OwnerIndex::build(&[element("App.Core.Archi", GateTier::Warn, &["src/archi/"])]);
    assert_eq!(
        owned_fqn(&directory_only.owner_of("src/archi.i.rs")),
        "App.Core.Archi",
        "只声明目录根 ⇒ 整组剥离候选命中"
    );

    let file_only =
        OwnerIndex::build(&[element("App.Core.Archi", GateTier::Warn, &["src/archi.rs"])]);
    assert_eq!(
        owned_fqn(&file_only.owner_of("src/archi.i.rs")),
        "App.Core.Archi",
        "只声明文件根 ⇒ 替换扩展名候选命中"
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

// ===========================================================================
// `module_roots`（`pub(crate)` 新增）：归一路径 → FQN 列表（树侧唯一来源）
// ===========================================================================

/// 正向（AC-5 / AC-6 / D2）：两写法归一同路径去重后恰 1 条 ⇒ 该节点判
/// `elementRoot`；键集为**归一形态**（无 `./`、无尾 `/`）。
#[test]
fn 模块根表按归一路径归并去重() {
    let index = OwnerIndex::build(&[
        element("A", GateTier::Warn, &["./src/auth/", "src/auth"]),
        element("B", GateTier::Warn, &["./src/billing/"]),
    ]);
    let roots = index.module_roots();
    let keys: Vec<&str> = roots.keys().copied().collect();
    assert_eq!(
        keys,
        vec!["src/auth", "src/billing"],
        "键集为归一形态（无前导 ./、无尾 /）且有序"
    );
    assert_eq!(
        roots.get("src/auth").cloned(),
        Some(vec!["A"]),
        "同元素重复声明去重后恰 1 ⇒ 判 elementRoot"
    );
    assert_eq!(roots.get("src/billing").cloned(), Some(vec!["B"]));
}

/// 异常（AC-5 / spec Scenario 同一归一路径被多元素声明）：表项 ≥ 2 且字典序 ⇒
/// 树侧据此判 `ambiguous`。
#[test]
fn 模块根表同路径多元素声明给出全量候选() {
    let index = OwnerIndex::build(&[
        element("App.Zeta", GateTier::Warn, &["./src/shared/"]),
        element("App.Alpha", GateTier::Warn, &["src/shared"]),
    ]);
    let roots = index.module_roots();
    assert_eq!(
        roots.get("src/shared").cloned(),
        Some(vec!["App.Alpha", "App.Zeta"]),
        "同归一路径多元素 ⇒ 表项字典序全量（len >= 2 判 ambiguous）"
    );
}

/// 异常（AC-5 / spec Scenario 容器元素零节点）：`path = []` 的元素不产任何表项。
#[test]
fn 模块根表对容器元素零表项() {
    let index = OwnerIndex::build(&[
        element("App", GateTier::Warn, &[]),
        element("App.Auth", GateTier::Warn, &["./src/auth/"]),
    ]);
    let roots = index.module_roots();
    assert_eq!(roots.len(), 1, "只有声明了 path 的元素进表");
    assert!(
        !roots.values().any(|fqns| fqns.contains(&"App")),
        "容器元素零表项"
    );
}

/// 边界：空路径条目（`""` / `"./"` 归一为空）被丢弃；空模型返回空表不 panic。
#[test]
fn 模块根表过滤空路径与空模型() {
    let index = OwnerIndex::build(&[
        element("App.Empty1", GateTier::Warn, &[""]),
        element("App.Empty2", GateTier::Warn, &["./"]),
    ]);
    assert!(
        index.module_roots().is_empty(),
        "空条目归一为空后丢弃（不产兜底表项）"
    );

    let empty = OwnerIndex::build(&[]);
    assert!(empty.module_roots().is_empty(), "空模型返回空表");
}

// ===========================================================================
// `contains_count`（`pub(crate)` 新增）：严格前缀后代模块数
// ===========================================================================

/// 正向（AC-5 / D6③）：严格前缀 `"{path}/"` 的后代模块数（按 FQN 去重、不含自身）。
#[test]
fn 后代计数按严格前缀统计() {
    let index = OwnerIndex::build(&[
        element("Src", GateTier::Warn, &["src/"]),
        element("A", GateTier::Warn, &["src/a/"]),
        element("B", GateTier::Warn, &["src/b/"]),
        element("A.Deep", GateTier::Warn, &["src/a/deep/"]),
    ]);
    assert_eq!(
        index.contains_count("src"),
        3,
        "src 的后代模块三个（不含自身）"
    );
    assert_eq!(index.contains_count("src/a"), 1, "src/a 的后代模块一个");
    assert_eq!(index.contains_count("src/b"), 0, "src/b 无后代模块");
    assert_eq!(index.contains_count("src/a/deep"), 0, "叶子元素根无后代");
}

/// 正向（AC-5 / spec Scenario 多 path 元素与 contains 去重）：一元素的四条文件形
/// 路径只计 1（按 FQN 去重）。
#[test]
fn 后代计数按元素去重() {
    let index = OwnerIndex::build(&[element(
        "App.Frontend.Shell",
        GateTier::Warn,
        &[
            "src/shell/index.ts",
            "src/shell/app.ts",
            "src/shell/routes.ts",
            "src/shell/store.ts",
        ],
    )]);
    assert_eq!(
        index.contains_count("src/shell"),
        1,
        "四条路径同属一元素 ⇒ 计 1（按 FQN 去重）而非 4"
    );
    assert_eq!(index.contains_count("src"), 1, "祖先层同样按元素去重");
}

/// 异常（AC-5 / spec 判据③）：非严格前缀不计数——`src/ab` 不以 `src/a/` 开头。
#[test]
fn 后代计数不误计非严格前缀() {
    let index = OwnerIndex::build(&[element("Ab", GateTier::Warn, &["src/ab/"])]);
    assert_eq!(
        index.contains_count("src/a"),
        0,
        "`src/ab` 不以 `src/a/` 开头（MUST NOT 用裸 starts_with 前缀）"
    );
    assert_eq!(index.contains_count("src"), 1, "上一级照常计数");
}

/// 边界：非归一入参先归一（`./` / `\` / 尾 `/` 三写法等价）；空串与 `.` 返回 0。
#[test]
fn 后代计数入参先归一且空值零结果() {
    let index = OwnerIndex::build(&[element("A", GateTier::Warn, &["src/a/"])]);
    for probe in ["./src/", "src\\", "src/"] {
        assert_eq!(
            index.contains_count(probe),
            1,
            "{probe:?} 归一后与 src 等价"
        );
    }
    for probe in ["", "."] {
        assert_eq!(index.contains_count(probe), 0, "{probe:?} 返回 0 不 panic");
    }
}

/// 边界：自身不计数、1000 个互不重叠的后代根容量足够、空模型恒 0。
#[test]
fn 后代计数上界与空模型() {
    let single = OwnerIndex::build(&[element("Only", GateTier::Warn, &["src/"])]);
    assert_eq!(single.contains_count("src"), 0, "自身不计入后代数");

    let mut elements: Vec<ArchiElement> = vec![element("Src", GateTier::Warn, &["src/"])];
    for index in 0..1000 {
        elements.push(element(
            &format!("Src.K{index:04}"),
            GateTier::Warn,
            &[format!("src/k{index:04}/").as_str()],
        ));
    }
    let wide = OwnerIndex::build(&elements);
    assert_eq!(
        wide.contains_count("src"),
        1000,
        "1000 个后代根在 u32 容量内不溢出"
    );

    let empty = OwnerIndex::build(&[]);
    assert_eq!(empty.contains_count("src"), 0, "空模型任意 path 皆 0");
}

// ===========================================================================
// 判据面单源（AC-6）：源码自证
// ===========================================================================

/// 异常（AC-6）：`strip_generated` 零残留、`strip_candidates` 恰一处定义 + 一处调用、
/// 三个新 helper 各恰一处、三处零改动件签名逐字未变。
#[test]
fn 判定面单源与零改动件签名() {
    let source = include_str!("owner.rs");
    assert_eq!(
        source.matches("strip_generated").count(),
        0,
        "旧剥离函数零残留（并集判据已取代）"
    );
    assert_eq!(
        source.matches("fn strip_candidates").count(),
        1,
        "候选构造函数恰一处定义"
    );
    assert_eq!(
        source.matches("strip_candidates(").count(),
        2,
        "候选构造函数恰一处定义 + 一处调用（owner_of 内）"
    );
    for helper in [
        "fn module_roots",
        "fn contains_count",
        "fn longest_prefix_hits",
        "fn normalize",
        "fn prefix_matches",
        "pub fn build",
        "pub fn owner_of",
    ] {
        assert_eq!(source.matches(helper).count(), 1, "{helper} 恰一处定义");
    }
    // 零短路结构：候选循环体内不得出现早退
    assert!(
        source.contains("for candidate in strip_candidates("),
        "候选集合并定档的循环在案（MUST NOT 短路）"
    );
    assert!(
        source.contains("pub(crate) fn module_roots(&self) -> BTreeMap<&str, Vec<&str>>"),
        "模块根表签名（树侧唯一来源）"
    );
    assert!(
        source.contains("pub(crate) fn contains_count(&self, path: &str) -> u32"),
        "后代计数签名"
    );
}
