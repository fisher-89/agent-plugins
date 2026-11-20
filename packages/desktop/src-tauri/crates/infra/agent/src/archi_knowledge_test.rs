//! `archi_knowledge` 的单元测试：知识文本的分层收口（骨架只留不可脚本化的散文与
//! 一行工具指路，示例 TOML 与字段形态归 `archi_scaffold`）、A 档指路句逐句在场
//! （含自检纪律两句：结构诊断带 `file:line`、生成物重生成命令名）、派生段收缩为
//! 四面词表 + 两处目录，以及拼装 / 降级 / 体量锚（实测值冻结 2139 B / 2629 B）。
//!
//! 零 mock：`include_str!` 取编译期内嵌的同一份字节（禁测试内复制副本）；
//! `layout` 派生与 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` /
//! `DiagnosticKind::ALL` 取真实常量。

use archi::dto::{ApiStatus, DiagnosticKind, ElementKind, GateTier};

const SKELETON: &str = include_str!("archi-knowledge.md");

/// 骨架文本（静态资产本体）。
fn skeleton() -> &'static str {
    SKELETON
}

/// 拼装产物（骨架 + 派生段）——`knowledge()` 的唯一出口。
fn text() -> String {
    crate::archi_knowledge::knowledge().expect("真实骨架应拼装成功")
}

// ---------------------------------------------------------------------------
// 分层删除面：示例围栏 / 字段语法教学 / entry 三档表一律缺场
// ---------------------------------------------------------------------------

/// 正向：骨架零代码围栏、零示例 TOML 段、零 `entry` 三档默认长表（教学面已归
/// `archi_scaffold` 与 `resolvedEntry`）。
#[test]
fn 骨架零围栏零示例零三档表() {
    let source = skeleton();
    assert_eq!(
        source.matches("```").count(),
        0,
        "零代码围栏（示例 TOML 归 archi_scaffold 的 draft）"
    );
    assert!(
        !source.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("[element.") || line.starts_with("[[relation]]")
        }),
        "骨架零可解析 TOML 块（键形只作行内引述）"
    );
    for piece in ["三档", "默认候选一", "默认候选二", "候选一", "候选二"] {
        assert!(!source.contains(piece), "零 entry 三档长表：`{piece}`");
    }
}

/// 正向（表驱动）：零字段语法教学段——逐字段形态教学句缺场（`kind = ` / `gate = `
/// 一类赋值示例、`entry` 四条判据细节、`doc` 判据门限都不在骨架）。
#[test]
fn 骨架零字段形态教学() {
    let source = skeleton();
    for piece in [
        "kind = \"",
        "gate = \"",
        "api = \"",
        "entry = \"",
        "path = [\"",
        "doc = \"",
        "metadata = {",
        "MUST 是 repo 相对 POSIX",
        "apiUnexpected 的两条判据",
    ] {
        assert!(!source.contains(piece), "零字段形态教学句：`{piece}`");
    }
}

// ---------------------------------------------------------------------------
// 分层新增面：kind 四值语义 / 计数单源 / 看护配置
// ---------------------------------------------------------------------------

/// 正向：`kind` 四值语义逐值在场（包根 / 容器 / crate / 组件文件）。
#[test]
fn 骨架含kind四值语义() {
    let source = skeleton();
    for (value, meaning) in [
        ("`package`", "包根"),
        ("`domain`", "容器"),
        ("`module`", "crate"),
        ("`component`", "组件文件"),
    ] {
        assert!(source.contains(value), "kind 值 `{value}` 在场");
        assert!(source.contains(meaning), "kind 语义 `{meaning}` 在场");
    }
}

/// 正向：计数复述单源一行（元素 / 关系数量以 `archi_drift` 的 `model.*` 为准）。
#[test]
fn 骨架含计数单源一行() {
    let source = skeleton();
    assert!(source.contains("archi_drift"), "点名读数工具");
    assert!(
        source.contains("model.elements") && source.contains("model.relations"),
        "计数以 drift 报告字段为准"
    );
}

/// 正向：架构域看护配置一行（`ignore` / `exclude` 两键）。
#[test]
fn 骨架含看护配置一行() {
    let source = skeleton();
    assert!(source.contains("config.json"), "点名架构域配置文件");
    for key in ["ignore", "exclude"] {
        assert!(source.contains(key), "含两键之一 `{key}`");
    }
    assert!(source.contains("archi_drift"), "非法即由 drift 报错点名");
}

// ---------------------------------------------------------------------------
// A 档指路句（D17 机械表）：每句携带工具名 + 该工具面确有的字段
// ---------------------------------------------------------------------------

/// 正向（表驱动）：A 档指路句逐句在场——纪律句关键词 → 工具名（或既有命令）→
/// 该面确有的字段 / 口径（D17 机械表八行 + 自检纪律两句：结构诊断带 `file:line`、
/// 生成物过期的重生成命令名）。
#[test]
fn 骨架_指路表逐句在场() {
    let source = skeleton();
    let cases: [(&str, &str, &str); 10] = [
        ("缺 `path`", "archi_module", "hasPath"),
        ("entry", "archi_module", "resolvedEntry"),
        ("api = none", "archi_drift", "apiFindings"),
        ("orphanHierarchy", "archi_module", "orphanHierarchy"),
        ("字段序", "archi_scaffold", "取模板"),
        ("覆盖", "archi_drift", "读数"),
        ("两轴", "archi_api", "action = \"check\""),
        ("config.json", "archi_drift", "报错点名"),
        ("结构诊断", "archi_module", "file:line"),
        (
            "生成物过期",
            "pnpm api:export",
            "cargo run -p archi-api-gen -- generate",
        ),
    ];
    for (needle, tool, field) in cases {
        assert!(source.contains(needle), "指路句锚点 `{needle}` 在场");
        assert!(source.contains(tool), "指路句携带工具 / 命令 `{tool}`");
        assert!(source.contains(field), "指路句携带该面确有的 `{field}`");
    }
}

/// 组合：`file:line` 指路的对象面确有此形态——结构诊断文案单点
/// `archi::dto::graph_diagnostics_error`（`archi_module` 的坏模型回灌走同一入口）
/// 产出的行带 `file:line`，故骨架的指路句不是空话。
#[test]
fn 指路句_file_line由真实诊断投影背书() {
    let source = skeleton();
    assert!(source.contains("`file:line`"), "骨架点名 `file:line`");
    let diagnostics = [archi::dto::ArchiDiagnostic {
        kind: DiagnosticKind::OrphanHierarchy,
        message: String::new(),
        file: "01-auth.toml".to_owned(),
        line: 7,
        column: None,
    }];
    let text = archi::dto::graph_diagnostics_error(&diagnostics);
    assert!(
        text.contains("01-auth.toml:7"),
        "诊断文案带 `file:line`：{text}"
    );
    assert!(
        text.contains(DiagnosticKind::OrphanHierarchy.as_str()),
        "诊断文案带种类词：{text}"
    );
}

/// 组合：生成物重生成命令名是仓内真实存在的命令——`pnpm api:export` 与
/// `cargo run -p archi-api-gen -- generate` 同时在场（指路不指向不存在的命令）。
#[test]
fn 指路句重生成命令名在场() {
    let source = skeleton();
    assert!(
        source.contains("`pnpm api:export`"),
        "点名 `pnpm api:export`（反引号包裹的命令词）"
    );
    assert!(
        source.contains("`cargo run -p archi-api-gen -- generate`"),
        "点名 `cargo run -p archi-api-gen -- generate`"
    );
    assert!(
        source.contains("MUST NOT 手改生成物"),
        "同句带「MUST NOT 手改生成物」纪律"
    );
}

/// 组合：骨架点名的每个 `archi_*` 工具名都在挂载清单命中（指路不指向不存在的工具）。
#[test]
fn 骨架点名的archi工具都在挂载清单() {
    let source = skeleton();
    let mut named: Vec<&str> = Vec::new();
    for candidate in [
        "archi_module",
        "archi_search",
        "archi_api",
        "archi_drift",
        "archi_scaffold",
    ] {
        if source.contains(candidate) {
            named.push(candidate);
        }
    }
    assert!(!named.is_empty(), "骨架至少点名一个 archi 工具");
    for name in named {
        assert!(
            crate::sdk::mount::lookup(name).is_some(),
            "骨架点名的 {name} 应在挂载清单内"
        );
    }
    // 骨架未点名的 archi 工具——断言集合封闭（零第三个 archi_ 前缀词）
    let words: Vec<&str> = source
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .filter(|word| word.starts_with("archi_"))
        .collect();
    for word in words {
        assert!(
            crate::sdk::mount::lookup(word).is_some(),
            "骨架出现的 `{word}` 应为真实工具名"
        );
    }
}

// ---------------------------------------------------------------------------
// 派生段：四面词表 + 两处目录
// ---------------------------------------------------------------------------

/// 正向：派生段含模型目录与契约资产两处（`layout` 复算）、**不含**契约索引路径。
#[test]
fn 派生段两处目录且零索引() {
    let text = text();
    let model_dir = foundation::layout::archi_graph_model_rel_dir();
    let models_dir = foundation::layout::archi_models_rel_dir();
    assert!(text.contains(&model_dir), "含模型目录 {model_dir}");
    assert!(text.contains(&models_dir), "含契约资产目录 {models_dir}");
    assert!(
        !text.contains("INDEX.toml"),
        "派生段零契约索引（索引与契约分家）"
    );
    assert!(
        !text.contains(&format!(
            "{}/INDEX.toml",
            foundation::layout::archi_graph_rel_dir()
        )),
        "派生段零索引文件路径"
    );
}

/// 正向（表驱动）：四面词表逐值在场（kind / gate / api 状态 / 结构诊断九词）。
#[test]
fn 派生段四面词表逐值() {
    let text = text();
    for value in ElementKind::ALL {
        assert!(
            text.contains(value.as_str()),
            "kind 词 {} 在场",
            value.as_str()
        );
    }
    for value in GateTier::ALL {
        assert!(
            text.contains(value.as_str()),
            "gate 词 {} 在场",
            value.as_str()
        );
    }
    for value in ApiStatus::ALL {
        assert!(
            text.contains(value.as_str()),
            "api 状态 {} 在场",
            value.as_str()
        );
    }
    for value in DiagnosticKind::ALL {
        assert!(
            text.contains(value.as_str()),
            "诊断词 {} 在场",
            value.as_str()
        );
    }
    assert_eq!(DiagnosticKind::ALL.len(), 9, "结构诊断九词封闭");
}

/// 边界：派生段缺席即降级语义不变——`gate` / `api` 的缺省档由 `Default` 派生。
#[test]
fn 派生段缺省档由default派生() {
    let text = text();
    assert!(
        text.contains(&format!("新元素缺省 {}", GateTier::default().as_str())),
        "gate 缺省档标注"
    );
    assert!(
        text.contains(&format!("（缺省 {}）", ApiStatus::default().as_str())),
        "api 缺省档标注"
    );
}

// ---------------------------------------------------------------------------
// 静默陷阱保留（R1）与保留项
// ---------------------------------------------------------------------------

/// 边界（表驱动）：四条 C 档纪律逐条在场——带引号 FQN 单键、`metadata` 必须内联表、
/// 目录尾 `/`、容器 `path` 恒空。
#[test]
fn 骨架静默陷阱四条在场() {
    let source = skeleton();
    for needle in ["带引号的完整 FQN 单键", "内联表", "目录尾 `/`", "容器恒空"] {
        assert!(source.contains(needle), "C 档纪律 `{needle}` 在场");
    }
}

/// 边界：保留项——写面与可见性三句（直改主仓 / 禁止越界 / 画布写盘后不自动刷新）+
/// 历史会话续接一句。
#[test]
fn 骨架保留写面可见性与续接() {
    let source = skeleton();
    for needle in [
        "git diff",
        "禁止改架构目录之外的文件",
        "不会自动更新",
        "续接历史会话",
    ] {
        assert!(source.contains(needle), "保留项 `{needle}` 在场");
    }
}

// ---------------------------------------------------------------------------
// 体量锚与占位符纪律
// ---------------------------------------------------------------------------

/// 边界：骨架体量冻结在**实测值 2139 B**（design 目标区间 2000–2150 B），门限
/// ≤ 2252 B 仍在断言内——增删内容须同批更新冻结值，MUST NOT 以「贴近门限」冒充达标。
#[test]
fn 骨架体量锚() {
    let bytes = skeleton().trim_end().len();
    assert_eq!(
        bytes, 2139,
        "骨架体量实测 2139 B 已冻结；内容变更须同批更新本值与全文锚"
    );
    assert!(
        (2000..=2150).contains(&bytes),
        "骨架体量 {bytes} B 应落在 design 目标区间 2000–2150 B"
    );
    assert!(
        bytes <= 2252,
        "骨架体量 {bytes} B 超过门限 2252 B（新增内容请先删等量旧内容）"
    );
}

/// 边界：全文（骨架 + 派生段）体量冻结在**实测值 2629 B**（design 目标区间
/// 2550–2750 B），门限 ≤ 2867 B 仍在断言内。
#[test]
fn 全文体量锚() {
    let bytes = text().len();
    assert_eq!(
        bytes, 2629,
        "全文体量实测 2629 B 已冻结；内容变更须同批更新本值与骨架锚"
    );
    assert!(
        (2550..=2750).contains(&bytes),
        "全文体量 {bytes} B 应落在 design 目标区间 2550–2750 B"
    );
    assert!(bytes <= 2867, "全文体量 {bytes} B 超过门限 2867 B");
}

/// 边界：骨架零 `<>` 占位符（占位符即「路径未派生 / 字段未定」的形态）；占位形态
/// 只以反引号包裹的行内词出现，故零「裸尖括号对」。
#[test]
fn 骨架零尖括号占位() {
    let source = skeleton();
    assert!(!source.contains("<>"), "零 `<>` 占位");
    assert!(!source.contains("<FQN>"), "零 `<FQN>` 占位");
    assert!(!source.contains("{{"), "零模板插值");
    assert!(
        !source.contains('<') || source.contains('`'),
        "尖括号只出现在反引号包裹的行内词内"
    );
}

// ---------------------------------------------------------------------------
// 拼装 / 降级 / 模块 doc 纪律
// ---------------------------------------------------------------------------

/// 组合：`knowledge()` 全文 = `skeleton.trim_end()` + 派生段（前缀相等 + 真包含）。
#[test]
fn knowledge全文以骨架起手() {
    let full = text();
    let head = skeleton().trim_end();
    assert!(full.starts_with(head), "全文以骨架 trim_end 起手");
    assert!(full.len() > head.len(), "派生段确有追加");
    assert!(full.contains("结构诊断："), "派生段含四面词表段头");
}

/// 异常（表驱动）：骨架为空 / 仅空白 → `Err`（文案含「知识骨架为空」）；`knowledge()`
/// 不 panic（既有降级不变）。
#[test]
fn build空骨架err且knowledge不panic() {
    for skeleton in ["", "   \n\t\n"] {
        let error = crate::archi_knowledge::build(skeleton).expect_err("空骨架应 Err");
        assert!(error.contains("知识骨架为空"), "文案含成因：{error}");
    }
    assert!(
        crate::archi_knowledge::knowledge().is_some(),
        "真实骨架不降级"
    );
}

/// 异常：骨架含 `<>` → `build` 返回 `Err`（文案含「占位符」）。
#[test]
fn build占位符err含触发点() {
    let error =
        crate::archi_knowledge::build("# 标题\n\n路径 <> 未派生\n").expect_err("占位符应 Err");
    assert!(error.contains("占位符"), "文案含占位符：{error}");
}

/// 边界（源码自证）：模块 doc 纪律保留「尽力增强、不炸 run」一条。
#[test]
fn 模块doc保留降级纪律() {
    let source = include_str!("archi_knowledge.rs");
    assert!(
        source.contains("尽力增强、不炸 run"),
        "doc 纪律一条在案（降级语义不变）"
    );
    assert!(
        source.contains("include_str!(\"archi-knowledge.md\")"),
        "骨架经 `include_str!` 内嵌（编译期静态资产）"
    );
    assert!(!source.contains("契约索引"), "模块 doc 零契约索引残留");
}

// ===========================================================================
// 关系第四键 `apis`（`relation-api-dependency-check` / AC-11 / D20）
//
// 骨架只留一句纪律：`apis` 是**开放值集**的名字清单（与 `doc` / `entry` 同类），故它
// 不进任何派生词表；判据长表（`undeclaredApi` / `unusedDeclaration` 一类）也不得下沉
// 到骨架——实体读数一律看 `archi_drift`。
// ===========================================================================

/// 正向：关系键面由三键改四键——`（from / to / desc / apis）` 逐字在场，旧三键串缺场。
#[test]
fn 骨架关系四键在场() {
    let source = skeleton();
    assert!(
        source.contains("`from` / `to` / `desc` / `apis`"),
        "关系四键串逐字在场"
    );
    assert!(
        !source.contains("`from` / `to` / `desc`）"),
        "旧三键串缺场（不并列保留两套键面）"
    );
}

/// 边界：`apis` 纪律句含四要素——只写名字 / 可留空 / 生成物面留空 / 结论看
/// `archi_drift`。
#[test]
fn 骨架apis纪律四要素() {
    let source = skeleton();
    let line = source
        .lines()
        .find(|line| line.starts_with("- `apis`"))
        .expect("骨架含 `apis` 纪律行");
    for needle in ["只写名字", "可留空", "生成物面留空", "archi_drift"] {
        assert!(line.contains(needle), "纪律要素 `{needle}` 在场：{line}");
    }
}

/// 异常：零判据长表——`apis` 的值面 / 两桶判据词不进骨架（读数归 `archi_drift`）。
#[test]
fn 骨架零apis判据长表() {
    let source = skeleton();
    for piece in [
        "undeclaredApi",
        "unusedDeclaration",
        "docMissing",
        "usedNotDeclared",
    ] {
        assert!(!source.contains(piece), "零判据长表：`{piece}` 不得下沉");
    }
}

/// 边界：`apis` 与 `doc` / `entry` 同列（单值 / 开放值集语义句在场），且不进派生词表
/// ——派生段逐词取自封闭词汇常量，`apis` 不在其中。
#[test]
fn 骨架apis不入派生词表() {
    let full = text();
    let head_len = skeleton().trim_end().len();
    let derived = &full[head_len..];
    assert!(
        !derived.contains("apis"),
        "派生段零 `apis`（开放值集不进词表）"
    );
    for closed in ["kind", "gate", "api 状态", "结构诊断"] {
        assert!(derived.contains(closed), "封闭词表段仍在场：`{closed}`");
    }
}
