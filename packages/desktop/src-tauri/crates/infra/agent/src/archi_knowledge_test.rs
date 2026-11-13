//! `archi_knowledge` 与静态骨架 `archi-knowledge.md` 的单元测试：词表三面派生
//! （kind / gate / 结构诊断）与目录路径派生的**在场**断言、骨架自证（唯一示例
//! TOML 经真实 `archi` 解析面 + 图构建双零诊断）、体量上限与降级语义。
//!
//! 零 mock（沿 test-design「Mock策略」表）：词表常量与目录派生取真实函数，骨架
//! 取 `include_str!` 编译期内嵌的同一份字节，解析面取真实 `TomlParser` /
//! `ModelGraph`（替身会抹掉「词表漂移 / 示例写坏」的判据）。零 IO：不落盘、不
//! 建 tempdir。

use std::collections::BTreeSet;

use archi::dto::{DiagnosticKind, ElementKind, GateTier};
use archi::graph::ModelGraph;
use archi::model::Model;
use archi::parse::{SourceParser, TomlParser};

use crate::archi_knowledge::{build, knowledge};

/// 静态骨架字节（与生产 `include_str!` 同一路径同一份文件——禁测试内复制副本）。
const SKELETON: &str = include_str!("archi-knowledge.md");

/// 唯一入口产物（每条用例自取，零共享可变状态）。
fn text() -> String {
    knowledge().expect("真实骨架应拼装成功（词表 / 目录派生全在场）")
}

// ---------------------------------------------------------------------------
// `knowledge`：词表派生（kind / gate + 缺省 / 结构诊断九词）
// ---------------------------------------------------------------------------

/// 正向：`ElementKind::ALL` 四值逐值 `as_str()` 在场（四值各出现，不是只出现
/// 一个词）。
#[test]
fn knowledge词表派生kind四值逐值在场() {
    let text = text();
    assert!(
        text.contains("元素 kind："),
        "派生段含 kind 词表引导语，实际全文:\n{text}"
    );
    for kind in ElementKind::ALL {
        assert!(
            text.contains(kind.as_str()),
            "知识文本缺元素 kind 词 {}（词表由 ALL 派生，新增变体未同步即红）",
            kind.as_str()
        );
    }
    // 四值以单点连接形态（" / "）串成一行的证据：整段连接串逐字在场
    let joined = ElementKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<&str>>()
        .join(" / ");
    assert!(
        text.contains(&joined),
        "kind 词表应按 ALL 序以 \" / \" 连接（实得连接串 {joined}）"
    );
}

/// 正向：`GateTier::ALL` 三值逐值在场；`GateTier::default()`（`off`）与「缺省」
/// 语义措辞同行出现（新元素 `gate` 缺省 `off` 一节成立）。
#[test]
fn knowledge词表派生gate三值逐值在场与缺省锚() {
    let text = text();
    for gate in GateTier::ALL {
        assert!(
            text.contains(gate.as_str()),
            "知识文本缺 gate 档词 {}",
            gate.as_str()
        );
    }
    let fallback = GateTier::default().as_str();
    let line = text
        .lines()
        .find(|line| line.contains("gate 档："))
        .unwrap_or_else(|| panic!("派生段缺 gate 引导行，实际全文:\n{text}"));
    assert!(
        line.contains("缺省"),
        "gate 引导行应写明缺省语义，实际行: {line}"
    );
    assert!(
        line.contains(fallback),
        "gate 引导行应携缺省档 {fallback}，实际行: {line}"
    );
}

/// 正向：`DiagnosticKind::ALL` 九值逐值 `.as_str()` 在场（缺失即点名缺失词——
/// 「新增诊断词未同步」由 `ALL` 驱动，测试侧不另抄字面量清单）。
#[test]
fn knowledge词表派生诊断九词逐值在场() {
    let text = text();
    assert!(
        text.contains("结构诊断："),
        "派生段含结构诊断引导语，实际全文:\n{text}"
    );
    for kind in DiagnosticKind::ALL {
        assert!(
            text.contains(kind.as_str()),
            "知识文本缺结构诊断词 {}（九词由 ALL 派生，新增变体必须同补 ALL）",
            kind.as_str()
        );
    }
}

/// 异常防护：三份 `ALL` 为封闭集——长度锚（加值即红）+ 去重后长度不变（漏值 /
/// 重复值不可绕过）。
#[test]
fn knowledge词汇漂移守卫三组封闭集长度与去重() {
    assert_eq!(ElementKind::ALL.len(), 4, "元素 kind 封闭四值");
    assert_eq!(GateTier::ALL.len(), 3, "gate 封闭三值");
    assert_eq!(DiagnosticKind::ALL.len(), 9, "结构诊断封闭九值");

    let kinds: BTreeSet<&str> = ElementKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(kinds.len(), 4, "kind 四值互异（重复值不可绕过词表对拍）");
    let gates: BTreeSet<&str> = GateTier::ALL.iter().map(|gate| gate.as_str()).collect();
    assert_eq!(gates.len(), 3, "gate 三值互异");
    let diagnostics: BTreeSet<&str> = DiagnosticKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    assert_eq!(diagnostics.len(), 9, "结构诊断九值互异");
}

/// 正向：目录路径来自 `foundation::layout` 相对路径派生族的**逐字返回值**（两次
/// `contains`），且全文零 `\`（`/` 分隔，目录串不自写）。
#[test]
fn knowledge目录派生注入两路径逐字在场且零反斜杠() {
    let text = text();
    let models = foundation::layout::archi_models_rel_dir();
    let decisions = foundation::layout::archi_decisions_rel_dir();
    assert!(
        text.contains(&models),
        "知识文本缺模型目录派生串 {models}（目录 MUST NOT 手抄）"
    );
    assert!(
        text.contains(&decisions),
        "知识文本缺决策记录目录派生串 {decisions}"
    );
    assert!(
        !text.contains('\\'),
        "知识文本应为 `/` 分隔（模型可见文本），不得出现反斜杠"
    );
}

/// 边界：体量 ≤ 4KB（AC-4 上限）且全文零 `<>` 连续两字符（占位符即「路径未
/// 派生」的形态）。
#[test]
fn knowledge体量与占位符不超4kb且零尖括号占位() {
    let text = text();
    assert!(
        text.len() <= 4096,
        "知识文本应在 4KB 内（实际 {} 字节）",
        text.len()
    );
    assert!(!text.contains("<>"), "知识文本不得含 <> 占位符");
}

/// 边界：纯函数性——连续三次调用逐字相等（零 IO、零时间 / 环境依赖），且与
/// `build(真实骨架)` 逐字相等（唯一装配面，禁第二份拼装）。
#[test]
fn knowledge纯函数性三次调用逐字相等且与build真实骨架同源() {
    let first = text();
    let second = text();
    let third = text();
    assert_eq!(first, second, "同参重复调用结果逐字相等");
    assert_eq!(second, third, "第三次调用同样逐字相等");
    assert_eq!(
        build(SKELETON).expect("真实骨架可拼装"),
        first,
        "knowledge() 即 build(骨架) 的唯一装配面（禁第二份拼装）"
    );
}

// ---------------------------------------------------------------------------
// `build`：骨架校验 + 派生段追加形态
// ---------------------------------------------------------------------------

/// 正向：派生段在骨架之后（首个 kind 词位置 > 骨架首条静态条款位置），骨架与
/// 派生段之间恰以 `\n\n` 分隔（不重复空行堆叠）。
#[test]
fn build骨架顺序派生段在后且单空行分隔() {
    let text = build(SKELETON).expect("真实骨架可拼装");
    let skeleton_clause = text
        .find("格式约定")
        .unwrap_or_else(|| panic!("骨架静态条款应完整在场:\n{text}"));
    let derived = text
        .find("元素 kind：")
        .unwrap_or_else(|| panic!("派生段引导语应完整在场:\n{text}"));
    assert!(
        derived > skeleton_clause,
        "派生段应在骨架静态条款之后（骨架 {skeleton_clause} / 派生 {derived}）"
    );
    assert!(
        text.contains("\n\n元素 kind："),
        "骨架与派生段之间恰以一个空行分隔"
    );
    assert!(
        !text.contains("\n\n\n"),
        "不得出现连续空行堆叠（尾随空白已归一）"
    );
}

/// 边界：骨架尾带多个换行 / 空格 → `Ok`，结果不以连续空行结尾（`trim_end()`
/// 生效），派生段仍完整在场。
#[test]
fn build尾随空白归一三形态仍ok() {
    for (label, skeleton) in [
        ("多换行", "骨架正文\n\n\n"),
        ("尾空格", "骨架正文   "),
        ("混合空白", "骨架正文\n \n\t"),
    ] {
        let text = build(skeleton).unwrap_or_else(|error| panic!("{label} 应 Ok，实得 {error}"));
        assert!(
            !text.contains("\n\n\n"),
            "{label}: 结果不得含连续空行，实际:\n{text}"
        );
        assert!(
            !text.ends_with('\n'),
            "{label}: 结果不得以换行结尾（trim_end 归一）"
        );
        for kind in ElementKind::ALL {
            assert!(
                text.contains(kind.as_str()),
                "{label}: 派生段仍完整在场（缺 {}）",
                kind.as_str()
            );
        }
    }
}

/// 异常：空骨架 / 纯空白骨架 → `Err`，文案含「知识骨架为空」（trim 后为空即
/// 拒）。
#[test]
fn build空骨架err文案含知识骨架为空() {
    for skeleton in ["", "   \n\t "] {
        let error = build(skeleton).expect_err("空白骨架必须 Err");
        assert!(
            error.contains("知识骨架为空"),
            "空骨架文案应含「知识骨架为空」，实际: {error}"
        );
    }
}

/// 异常：含 `<>` → `Err`，文案点名「知识骨架含占位符」这一成因（含触发串与修
/// 法指引，不是「骨架为空」那一条）；`<archi_dir>`（单尖括号形态、无连续两字
/// 符）→ `Ok`（判据 = 连续两字符 `<>`）。
#[test]
fn build占位符err带触发点且单尖括号形态ok() {
    let error = build("框架 <>\n").expect_err("含 <> 占位符必须 Err");
    assert!(
        error.contains("知识骨架") && error.contains("占位符"),
        "占位符文案应点名「知识骨架 + 占位符」成因，实际: {error}"
    );
    assert!(
        !error.contains("为空"),
        "占位符分支与空骨架分支文案互斥（不得回落到「知识骨架为空」），实际: {error}"
    );
    assert!(
        build("<archi_dir>").is_ok(),
        "单 `<...>` 形态不触发判据（该形态的禁令由骨架章节用例承载）"
    );
}

/// 异常：降级语义（`.ok()`）——空 / 占位符骨架落 `None`（compose 侧只丢
/// `preamble` 位，run 不 `Err`）。
#[test]
fn 降级语义ok为none即丢preamble位() {
    assert_eq!(build("").ok(), None, "空骨架降级 None");
    assert_eq!(build("<>").ok(), None, "占位符骨架降级 None");
    assert!(knowledge().is_some(), "真实骨架不降级");
}

// ---------------------------------------------------------------------------
// 静态骨架 `archi-knowledge.md`：唯一示例围栏块 + 解析 / 图双重自证
// ---------------------------------------------------------------------------

/// 骨架内唯一 toml 围栏块的正文（起止配对、围栏内非空）。
fn example_fence() -> &'static str {
    let opens = SKELETON.matches("```toml").count();
    assert_eq!(opens, 1, "骨架内 toml 围栏起标记恰 1 处");
    assert_eq!(
        SKELETON.matches("```").count(),
        2,
        "围栏起止配对（恰两枚 ``` 且无未闭合围栏）"
    );
    let start = SKELETON.find("```toml").expect("围栏在场") + "```toml".len();
    let rest = &SKELETON[start..];
    let end = rest.find("```").expect("围栏闭合");
    let body = rest[..end].trim();
    assert!(!body.is_empty(), "围栏内非空（示例本体）");
    body
}

/// 骨架示例解析产物（真实 `TomlParser`，零 mock）。
fn example_model() -> (Model, Vec<archi::model::Diagnostic>) {
    let outcome = TomlParser::parse(example_fence());
    (outcome.model, outcome.diagnostics)
}

/// 正向：示例经 `TomlParser::parse` 零结构诊断（字段名 / 值域 / 内联
/// `metadata` / 键形任一漂移即红），且元素 ≥ 2 并含带点的精确 FQN。
#[test]
fn 示例解析自证零结构诊断且元素不少于二() {
    let (model, diagnostics) = example_model();
    assert!(
        diagnostics.is_empty(),
        "示例 TOML 应零解析诊断，实际: {diagnostics:?}"
    );
    assert!(
        model.elements.len() >= 2,
        "示例应声明至少两个元素（祖先 + 后代），实际: {:?}",
        model.elements.iter().map(|e| &e.fqn).collect::<Vec<_>>()
    );
    for fqn in ["App", "App.Auth"] {
        assert!(model.element(fqn).is_some(), "示例应含精确 FQN {fqn}");
    }
    assert!(
        model
            .element("App.Auth")
            .is_some_and(|e| e.fqn.contains('.')),
        "元素 FQN 精确含点（带引号完整 FQN 单键的落表形态）"
    );
}

/// 正向：示例图自证——`ModelGraph::build(model).diagnostics()` 零诊断（引用与
/// 层级自洽：零 unknownReference / orphanHierarchy / cycle / dependsOnAncestor）。
#[test]
fn 示例图自证零图诊断() {
    let (model, _) = example_model();
    let graph = ModelGraph::build(model);
    let diagnostics = graph.diagnostics();
    assert!(
        diagnostics.is_empty(),
        "示例模型建图应零结构诊断，实际: {diagnostics:?}"
    );
}

/// 异常防护：内联 `metadata` 锁点——`App.Auth` 的 `metadata` 含 `owner` 键，且元
/// 素集中不存在 FQN 形如 `App.Auth.metadata` 的条目（写成
/// `[element."X".metadata]` 子表即被读成另一元素 → 本断言红）。
#[test]
fn 内联metadata锁点owner键在场且无子表误读元素() {
    let (model, diagnostics) = example_model();
    assert!(diagnostics.is_empty(), "解析应零诊断: {diagnostics:?}");
    let auth = model
        .element("App.Auth")
        .unwrap_or_else(|| panic!("示例缺 App.Auth"));
    assert!(
        auth.metadata.contains_key("owner"),
        "内联 metadata 表应落 owner 键，实际: {:?}",
        auth.metadata
    );
    assert!(
        !model
            .elements
            .iter()
            .any(|element| element.fqn.ends_with(".metadata")),
        "不得出现 `*.metadata` 元素（子表写法会把表头读成另一元素），实际: {:?}",
        model.elements.iter().map(|e| &e.fqn).collect::<Vec<_>>()
    );
}

/// 异常防护：键形锁点——元素以带引号完整 FQN 单键落表（解析成功且 FQN 含点），
/// 同一元素不得以大小写变体双份出现。
#[test]
fn 键形锁点带引号完整fqn单键且无大小写双份() {
    let (model, _) = example_model();
    let derived: Vec<&str> = model
        .elements
        .iter()
        .map(|element| element.fqn.as_str())
        .filter(|fqn| fqn.contains('.'))
        .collect();
    assert!(
        !derived.is_empty(),
        "示例应至少有一个点分 FQN 元素（带引号完整 FQN 单键的正常形态）"
    );
    let lowered: BTreeSet<String> = model
        .elements
        .iter()
        .map(|element| element.fqn.to_lowercase())
        .collect();
    assert_eq!(
        lowered.len(),
        model.elements.len(),
        "同一元素不得以大小写变体双份出现，实际: {:?}",
        model.elements.iter().map(|e| &e.fqn).collect::<Vec<_>>()
    );
}

/// 正向：示例中 `gate` 取值 ∈ `GateTier::ALL`（逐值比对 `as_str()` 集）；不出现
/// 封闭集外档位串。
#[test]
fn gate词面取值落在封闭集内() {
    let (model, _) = example_model();
    let legal: BTreeSet<&str> = GateTier::ALL.iter().map(|gate| gate.as_str()).collect();
    for element in &model.elements {
        assert!(
            legal.contains(element.gate.as_str()),
            "元素 {} 的 gate 取值 {} 不在封闭集 {:?}",
            element.fqn,
            element.gate.as_str(),
            legal
        );
    }
}

/// 正向：每条 `relation` 的 `from` / `to` 均在元素集内，`desc` 非空（祖先 → 后代
/// 表达式由零 `dependsOnAncestor` 兜住）。
#[test]
fn 关系两端已声明且说明非空() {
    let (model, _) = example_model();
    assert!(
        !model.relations.is_empty(),
        "示例应至少声明一条关系（[[relation]] 条目形态）"
    );
    for relation in &model.relations {
        assert!(
            model.element(&relation.from).is_some(),
            "关系起点 {} 未声明",
            relation.from
        );
        assert!(
            model.element(&relation.to).is_some(),
            "关系终点 {} 未声明",
            relation.to
        );
        assert!(
            relation
                .description
                .as_deref()
                .is_some_and(|desc| !desc.trim().is_empty()),
            "关系说明非空（{from} → {to}）",
            from = relation.from,
            to = relation.to
        );
    }
    let graph = ModelGraph::build(model);
    assert!(
        graph.diagnostics().is_empty(),
        "祖先指向后代：零 dependsOnAncestor / cycle"
    );
}

/// 边界：`docs` 指向不存在的虚构路径不产生结构诊断（`docNotFound` 属 warning，
/// 不进 `diagnostics()` 自证面）。
#[test]
fn 文档引用不触发结构错() {
    let (model, _) = example_model();
    let has_docs = model
        .elements
        .iter()
        .any(|element| !element.docs.is_empty());
    assert!(has_docs, "示例应至少声明一条 docs 关联（引用形态可考）");
    let graph = ModelGraph::build(model);
    assert!(
        graph.diagnostics().is_empty(),
        "虚构 docs 路径只报 warning，不入结构诊断: {:?}",
        graph.diagnostics()
    );
}

/// 异常：骨架全文零 `<>` 连续两字符，零 `<change>` / `<phase>` / `{{ }}` /
/// `__INCLUDE:*` 一类模板片。
#[test]
fn 零占位符全文无模板片() {
    assert!(!SKELETON.contains("<>"), "骨架不得含 <> 占位符");
    for pattern in ["<change>", "<phase>", "{{", "}}", "__INCLUDE:"] {
        assert!(!SKELETON.contains(pattern), "骨架不得含模板片 {pattern:?}");
    }
}

/// 异常：骨架全文不含 `openspec` / `architecture` / `models/` / `decisions` 字面
/// （目录路径由 `build()` 派生注入，AC-14）。
#[test]
fn 零目录字面量全文无架构域路径串() {
    for literal in ["openspec", "architecture", "models/", "decisions"] {
        assert!(
            !SKELETON.contains(literal),
            "骨架不得手抄目录字面量 {literal:?}（由 layout 派生注入）"
        );
    }
}

/// 边界：体量切分——骨架 ≤ 3KB（「骨架 ≤ 3KB + 派生段 ≤ 1KB」余量）。
#[test]
fn 体量切分骨架不超3kb() {
    assert!(
        SKELETON.len() <= 3 * 1024,
        "骨架应在 3KB 内（实际 {} 字节）",
        SKELETON.len()
    );
}

/// 正向：proposal「必静态部分」八条逐条关键词在场（缺任一条即红）。
#[test]
fn 必静态条款在场八条关键词逐条() {
    let clauses: [(&str, [&str; 2]); 8] = [
        (
            "① 带引号完整 FQN 单键 / 禁点分嵌套",
            ["带引号的完整 FQN 单键", "禁点分嵌套"],
        ),
        ("② 内联 metadata 表 / 禁子表", ["内联表", "子表"]),
        ("③ 最长前缀层级 + 祖先先声明", ["最长前缀", "先声明祖先"]),
        ("④ 新元素 gate 缺省不收紧", ["gate", "缺省"]),
        (
            "⑤ 不写接口与导出符号（只经 docs 引用）",
            ["不写接口", "docs"],
        ),
        (
            "⑥ 改完调 archi_module 复查并按 file:line 修",
            ["archi_module", "file:line"],
        ),
        (
            "⑦ 直改主仓 + 随 git diff 评审 + 写盘后请用户点刷新",
            ["直改主仓", "git diff"],
        ),
        ("⑧ 续接历史会话先取当下模型", ["续接历史会话", "当下模型"]),
    ];
    for (label, keywords) in clauses {
        for keyword in keywords {
            assert!(
                SKELETON.contains(keyword),
                "必静态条款「{label}」缺关键词 {keyword:?}"
            );
        }
    }
}

// ===========================================================================
// `api` 状态面（`module-api-contract-format` / D25）
//
// 派生段追加一行 `api 状态：<ApiStatus::ALL 逐值以 " / " 连接>（缺省 declared）`，
// 词表从 `ALL` 派生、MUST NOT 手抄；目录面追加契约目录相对路径。
// ===========================================================================

use archi::dto::ApiStatus;

/// 正向（AC-10 / D25）：`ApiStatus::ALL` 两值逐值 `as_str()` 在场，且连接形态与
/// 其余三面同源（`" / "` 分隔、缺省 `declared` 锚在括号内）。
#[test]
fn knowledge词表派生api状态二值逐值在场() {
    let text = text();
    for status in ApiStatus::ALL {
        assert!(
            text.contains(status.as_str()),
            "知识文本缺 api 状态词 {:?}（词表 MUST NOT 手抄）",
            status.as_str()
        );
    }
    let expected = ApiStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<&str>>()
        .join(" / ");
    assert!(
        text.contains(&format!("api 状态：{expected}")),
        "连接形态须与 kind / gate / 诊断三面同源（`\" / \"` 分隔）：{expected}"
    );
    assert!(
        text.contains(&format!("（缺省 {}）", ApiStatus::default().as_str())),
        "缺省档锚定在括号内（`ApiStatus::default()` 派生）"
    );
}

/// 正向（AC-10）：契约目录相对路径由 `archi_api_rel_dir()` 逐字注入；三棵目录前缀
/// 同源（`archi_rel_dir()` 共享前缀）。
#[test]
fn knowledge契约目录派生注入且三目录同源() {
    let text = text();
    let api = foundation::layout::archi_api_rel_dir();
    assert!(
        text.contains(&api),
        "知识文本缺契约目录派生串 {api}（目录 MUST NOT 手抄）"
    );
    for relative in [
        foundation::layout::archi_models_rel_dir(),
        foundation::layout::archi_decisions_rel_dir(),
        api.clone(),
    ] {
        assert!(
            relative.starts_with(&foundation::layout::archi_rel_dir()),
            "{relative} 应以架构域相对根为前缀（同源派生）"
        );
    }
}

/// 边界（D25）：`api` 状态面与 gate 档面词集互不相交（`declared` / `none` 不出现在
/// gate 词集，`off` / `warn` / `strict` 不出现在 api 词集）；四面去重后长度不变。
#[test]
fn knowledge词集四面不互相包含() {
    let api_words: BTreeSet<&str> = ApiStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect();
    let gate_words: BTreeSet<&str> = GateTier::ALL.iter().map(|gate| gate.as_str()).collect();
    assert_eq!(api_words.len(), 2, "api 状态封闭二值");
    assert_eq!(gate_words.len(), 3, "gate 封闭三值");
    assert!(
        api_words.is_disjoint(&gate_words),
        "api 词集与 gate 词集 MUST 不相交：{api_words:?} / {gate_words:?}"
    );

    let kind_words: BTreeSet<&str> = ElementKind::ALL.iter().map(|kind| kind.as_str()).collect();
    let diagnostic_words: BTreeSet<&str> = DiagnosticKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    for (label, words) in [
        ("api", &api_words),
        ("gate", &gate_words),
        ("kind", &kind_words),
        ("diagnostic", &diagnostic_words),
    ] {
        let mut copy: Vec<&&str> = words.iter().collect();
        copy.sort_unstable();
        copy.dedup();
        assert_eq!(copy.len(), words.len(), "{label} 面去重后长度不变");
    }
}

/// 边界：`ApiStatus` 线词与 `ArchiElement` 序列化文本的 `"api"` 字段值逐字相等
/// （两面对拍，零手抄词表）。
#[test]
fn api_status线词与元素投影串相等() {
    use archi::dto::{ArchiElement, ElementKind, GateTier};
    use std::collections::BTreeMap;

    for status in ApiStatus::ALL {
        let element = ArchiElement {
            fqn: "App.Core.Mod".to_owned(),
            kind: ElementKind::Module,
            gate: GateTier::Warn,
            api: status,
            path: vec!["src/mod/".to_owned()],
            summary: None,
            metadata: BTreeMap::new(),
            model_file: ".dev-team/architecture/models/01.toml".to_owned(),
            model_line: 1,
            mark: archi::dto::NodeMark::None,
        };
        let value = serde_json::to_value(&element).expect("序列化失败");
        assert_eq!(
            value["api"],
            serde_json::json!(status.as_str()),
            "元素投影串的 api 字段与 as_str() 逐字相等（{status:?}）"
        );
    }
}

/// 边界：`build` 的骨架降级判据零变化——空骨架 / 含 `<>` 占位符仍 `Err`（新增 api
/// 行不引入新降级路径）。
#[test]
fn build降级判据零变化() {
    assert!(build("   ").is_err(), "空骨架仍 Err（降级语义零变化）");
    let placeholder = format!("示例：{}", "<>");
    let error = build(&placeholder).expect_err("含 <> 占位符仍 Err");
    assert!(error.contains("占位符"), "文案含触发点：{error}");
}

/// 边界：知识文本体量上限随 api 行与契约目录小幅增长后仍 ≤ 4KB。
#[test]
fn knowledge体量仍不超4kb() {
    let text = text();
    assert!(
        text.len() <= 4096,
        "体量上限 4096 字节（实得 {} 字节）",
        text.len()
    );
}

/// 边界：纯函数性——三次调用逐字相等（含新增 api 行与契约目录）。
#[test]
fn knowledge三次调用逐字相等() {
    let first = text();
    let second = text();
    let third = text();
    assert_eq!(first, second, "三次调用逐字相等");
    assert_eq!(second, third, "三次调用逐字相等");
}

/// 边界：真实骨架（`include_str!`）与 `build` 同源——`build(SKELETON)` 的产物含
/// api 状态行与契约目录（骨架零改动时的最小闭合）。
#[test]
fn build真实骨架产物含api面() {
    let built = build(SKELETON).expect("真实骨架应拼装成功");
    assert!(
        built.contains("api 状态："),
        "派生段含 api 状态行：\n{built}"
    );
    assert!(
        built.contains(&foundation::layout::archi_api_rel_dir()),
        "派生段含契约目录：\n{built}"
    );
    for status in ApiStatus::ALL {
        assert!(
            built.contains(status.as_str()),
            "api 状态词 {:?} 在场",
            status.as_str()
        );
    }
}

// ===========================================================================
// 骨架 md 的两处改写（`module-api-contract-format` / D25）：字段面补 `api`、
// 「边界」段换成「契约权威在契约资产」口径。
//
// 判据措辞口径（与设计 `:949` / `:983` 的对齐说明）：设计写「契约权威在…**api 目录**」，
// 骨架 md 的实现措辞是「契约权威在**契约资产**（末段给出路径）」。二者不是同一字面——
// 设计那半句是**语义要求**（权威指向契约资产、路径由派生段给出），md 侧按「零目录字面量」
// 既有纪律只能写「契约资产」+「末段给出路径」，「api 目录」这个具体串由 `build()` 的派生段
// 注入（`契约目录：<archi_api_rel_dir()>`）。故本组用例把判据落成三条可机械核对的形态：
// ① 旧口径零命中；② 新语义的**关键词**在场（契约资产 / 数据末给出路径 / 状态指针 /
// 正文不进模型）；③ 「api 目录」这一路径串由派生段承载而非骨架手抄。
// ===========================================================================

/// 正向（AC-10 / D25）：`格式约定` 段的元素字段枚举行含 `api`，且同处给出二值
/// `declared`（缺省）/ `none`；六项既有字段`kind` / `gate` / `path` / `docs` /
/// `summary` / `metadata` 仍在（追加非替换）。
#[test]
fn 骨架字段面补api且六项仍在() {
    let field_line = SKELETON
        .lines()
        .find(|line| line.contains("元素字段面："))
        .unwrap_or_else(|| panic!("骨架缺「元素字段面：」枚举行：\n{SKELETON}"));
    assert!(
        field_line.contains("`api`"),
        "字段枚举行须含反引号形态的 `api`：{field_line}"
    );
    // 六项既有字段逐项仍在（追加非替换）
    for field in [
        "`kind`",
        "`gate`",
        "`path`",
        "`docs`",
        "`summary`",
        "`metadata`",
    ] {
        assert!(
            field_line.contains(field),
            "既有字段 {field} 须仍在枚举行（追加非替换）：{field_line}"
        );
    }
    assert!(
        field_line.contains("必填"),
        "`kind` 的必填语义须仍在：{field_line}"
    );

    // 二值定义同行给出：`declared`（缺省）/ `none`（显式无 API）
    let api_line = SKELETON
        .lines()
        .find(|line| line.contains("状态指针"))
        .unwrap_or_else(|| panic!("骨架缺 `api` 语义行：\n{SKELETON}"));
    assert!(
        api_line.contains("`declared`"),
        "缺省值 declared：{api_line}"
    );
    assert!(api_line.contains("`none`"), "另一值 none：{api_line}");
    assert!(api_line.contains("缺省"), "declared 标注为缺省：{api_line}");

    // 二值从代码侧词表逐值在场（词表不手抄的对照面）
    for status in ApiStatus::ALL {
        assert!(
            SKELETON.contains(status.as_str()),
            "骨架须写出 api 线词 {:?}（与 `ApiStatus::ALL` 同源）",
            status.as_str()
        );
    }
}

/// 正向（D25）：旧口径「契约权威在对应能力的 spec 文本」**零命中**；新表述含
/// 「契约资产」与「末段给出路径」「模型只存状态指针」「签名 / 导出符号 / 接口正文
/// 不进模型」三段语义（四条断言）。
#[test]
fn 骨架边界段改写为契约资产口径() {
    assert!(
        !SKELETON.contains("契约权威在对应能力的 spec 文本"),
        "旧口径 MUST 零命中（本变更正是要替换它）"
    );

    let authority = SKELETON
        .lines()
        .find(|line| line.contains("契约权威"))
        .unwrap_or_else(|| panic!("骨架缺「契约权威」行：\n{SKELETON}"));
    assert!(
        authority.contains("契约资产"),
        "权威指向契约资产：{authority}"
    );
    assert!(
        authority.contains("末段给出路径"),
        "路径由派生段给出（骨架零目录字面量的等价形态）：{authority}"
    );
    assert!(
        authority.contains("模型只存状态指针"),
        "模型只存状态指针 `api`：{authority}"
    );
    assert!(
        authority.contains("MUST NOT 抄契约正文"),
        "契约正文不进模型：{authority}"
    );

    let boundary = SKELETON
        .lines()
        .find(|line| line.contains("模型里不写接口"))
        .unwrap_or_else(|| panic!("骨架缺「模型里不写接口」行：\n{SKELETON}"));
    for keyword in ["函数签名", "类型定义", "导出清单", "不进模型正文"] {
        assert!(
            boundary.contains(keyword),
            "签名 / 导出符号 / 接口正文不进模型正文（缺 {keyword:?}）：{boundary}"
        );
    }

    // 「api 目录」这一具体路径串位于**派生段**（骨架手抄会与 `零目录字面量` 冲突）
    let derived = text();
    assert!(
        derived.contains(&foundation::layout::archi_api_rel_dir()),
        "契约目录路径由派生段承载：{derived}"
    );
    assert!(
        derived.contains("契约目录："),
        "派生段须有契约目录引导语：{derived}"
    );
}

/// 异常（AC-17 / 自查面）：骨架 md 内零 `crate` / `Cargo.toml` 字样；零 `api 状态：`
/// 手写行与零 `元素 kind：` 手写行（词表只从代码派生，不落 md）；零 `<>` 占位符。
#[test]
fn 骨架语言词与派生行禁写() {
    for forbidden in ["crate", "Cargo.toml"] {
        assert!(
            !SKELETON.contains(forbidden),
            "骨架 MUST NOT 含语言词 {forbidden:?}（AC-17）"
        );
    }
    // 派生段的四个引导语一律不得在骨架里手抄（否则词表会双写漂移）
    for hand_written in ["api 状态：", "元素 kind：", "gate 档：", "结构诊断："] {
        assert!(
            !SKELETON.contains(hand_written),
            "骨架 MUST NOT 手抄派生行 {hand_written:?}（词表只从代码派生）"
        );
    }
    assert!(!SKELETON.contains("<>"), "骨架不得含 <> 占位符");
}

// ---------------------------------------------------------------------------
// 本变更：`archi_api` 自检纪律（两轴 + 生成物用既有命令重生成、MUST NOT 手改）
// ---------------------------------------------------------------------------

/// 正向（AC-16 / D21）：知识文本含 `archi_api` 自检句——点名两轴（生成物三类差异 /
/// 契约判据）与重生成命令（`pnpm api:export` 或 `cargo run -p archi-api-gen -- generate`），
/// 且与 `include_str!("archi-knowledge.md")` 同源（零测试内副本）。
#[test]
fn knowledge含archi_api自检纪律() {
    let text = text();
    for piece in [
        "archi_api",
        "生成物",
        "契约判据",
        "pnpm api:export",
        "MUST NOT 手改生成物",
    ] {
        assert!(text.contains(piece), "知识文本含 `{piece}`");
    }
    // 同源：静态骨架里确有这段（拼装只做词表 / 路径派生，不改写散文）
    assert!(
        SKELETON.contains("archi_api"),
        "骨架字节含 archi_api（零测试内副本）"
    );
    assert!(
        SKELETON.contains("MUST NOT 手改生成物"),
        "骨架字节含硬线（生成物禁手改）"
    );
}

/// 异常（AC-16 / spec「零 `<>` 占位符」）：全文零 `<>`、零 `{{`、零 `__INCLUDE:`；
/// `trim()` 非空；体量上限内（既有 4KB 上限）。
#[test]
fn knowledge零占位符与体量上限() {
    let text = text();
    assert!(!text.contains("<>"), "零 `<>` 占位符");
    assert!(!text.contains("{{"), "零 `{{{{` 模板位");
    assert!(!text.contains("__INCLUDE:"), "零 include 指令残留");
    assert!(!text.trim().is_empty(), "产物非空");
    assert!(
        text.len() <= 4096,
        "体量在既有上限内（{} 字节）",
        text.len()
    );
    // 新增恰好一条 bullet 级增量（不是整段重写）：`archi_api` 句独占一行
    let bullet = text
        .lines()
        .find(|line| line.contains("archi_api"))
        .expect("自检句在案");
    assert!(
        bullet.trim_start().starts_with('-'),
        "自检纪律以 bullet 形态追加：{bullet}"
    );
}
