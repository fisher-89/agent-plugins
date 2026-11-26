//! `archi_knowledge` / `archi-knowledge.md` 的单元测试：知识面收窄后的五段结构（三资产
//! 分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断 + 兼容一句）、零语法教学（零示例
//! 围栏 / 零键形示例 / 零字段值域行 / 零 `metadata` 示例）、零结构诊断可执行词、双门限
//! 体量冻结（骨架 ≤ 1800 B / 全文 ≤ 2000 B）、两处派生目录仍在行程、指路只指向真实工具、
//! 旧立场零残留，以及拼装 / 降级语义零回归。
//!
//! 零 mock：`include_str!("archi-knowledge.md")` 取编译期内嵌的同一份字节（禁测试内
//! 复制副本）；`knowledge()` 走真实 `build` + 真实 `foundation::layout` 派生；
//! `mount::registry()` 是工具名真值（指路不指向不存在工具）。

use archi::dto::DiagnosticKind;

const SKELETON: &str = include_str!("archi-knowledge.md");

/// 骨架文本（静态资产本体）。
fn skeleton() -> &'static str {
    SKELETON
}

/// 拼装产物（骨架 + 派生段）——`knowledge()` 的唯一出口。
fn text() -> String {
    crate::archi_knowledge::knowledge().expect("真实骨架应拼装成功")
}

/// 体量按 LF 归一后测量：仓库行尾策略是 `eol=lf`，但工作树可能残留 CRLF 检出——行尾
/// 差异不得改变冻结值。
fn lf_bytes(text: &str) -> usize {
    text.replace("\r\n", "\n").len()
}

/// 旧立场的禁止短语（按片段拼接：产品源码面对这些整串零字面命中——AC-12a 含测试
/// 夹具面；判据仍在，禁的是「字面持有」）。
fn stale_stances() -> Vec<String> {
    vec![
        ["无专用", "写工具"].concat(),
        ["不设专用", "写工具"].concat(),
        ["MUST NOT 存在", "专用写"].concat(),
        ["写盘仍用", "既有"].concat(),
    ]
}

// ---------------------------------------------------------------------------
// 五段结构与手改出口
// ---------------------------------------------------------------------------

/// 正向（矩阵 5 组锚）：五段小节标题在场；三资产分工三句在场；手改出口一句含三要素
/// （「工具表达不了的」/「手改」/ `archi_module` 自查 / `file:line`）；工具面含五只读名 +
/// 两写工具名 + `archi_api` 三动作。
#[test]
fn 五段逐句在场() {
    let source = skeleton();
    for heading in [
        "# 架构模型须知",
        "## 三资产分工",
        "## 设计顺序",
        "## 三条不可越界",
        "## 工具面",
        "## 留给判断",
    ] {
        assert!(source.contains(heading), "小节标题 `{heading}` 在场");
    }

    // 三资产分工三句：TOML 拓扑 / schema 接触面（唯一凭据）/ md 理由（无结构）
    for piece in ["TOML 答拓扑", "schema（唯一凭据）答接触面", "md 答理由"] {
        assert!(source.contains(piece), "三资产分工 `{piece}` 在场");
    }

    let exit_line = source
        .lines()
        .find(|line| line.contains("工具表达不了的"))
        .expect("手改出口一句在场");
    for piece in ["手改", "archi_module", "file:line"] {
        assert!(exit_line.contains(piece), "出口句含 `{piece}`：{exit_line}");
    }

    for tool in [
        "archi_search",
        "archi_module",
        "archi_drift",
        "archi_api",
        "archi_scaffold",
        "archi_element",
        "archi_relation",
    ] {
        assert!(source.contains(tool), "工具面含 `{tool}`");
    }
}

// ---------------------------------------------------------------------------
// 零语法教学 / 零结构诊断词
// ---------------------------------------------------------------------------

/// 异常（矩阵 4 面）：语法教学段全离场——零可解析 TOML 围栏、零键形示例、零字段值域
/// 行、零 `metadata = { … }` 示例（形态归 `archi_scaffold` 词典与写工具参数描述）。
#[test]
fn 语法教学段全离场() {
    let source = skeleton();
    assert_eq!(source.matches("```").count(), 0, "零代码围栏");
    for stale in ["[element.\"", "[[relation]]", "metadata = {", "path = ["] {
        assert!(!source.contains(stale), "零键形示例 `{stale}`");
    }
    for line in source.lines() {
        let trimmed = line.trim_start();
        for key in [
            "kind = ",
            "gate = ",
            "api = ",
            "entry = ",
            "doc = ",
            "summary = ",
        ] {
            assert!(!trimmed.starts_with(key), "零字段值域行（`{key}`）：{line}");
        }
        assert!(
            !trimmed.starts_with("- `kind`"),
            "零 kind 四值 bullet 教学：{line}"
        );
    }
}

/// 异常（穷举诊断词）：结构诊断词零命中（诊断只由工具 `Err` 与 drift 读数承载）。
#[test]
fn 零结构诊断可执行词() {
    let source = skeleton();
    for kind in DiagnosticKind::ALL {
        assert!(
            !source.contains(kind.as_str()),
            "诊断词 `{}` 不得下沉骨架",
            kind.as_str()
        );
    }
}

// ---------------------------------------------------------------------------
// 体量门限
// ---------------------------------------------------------------------------

/// 边界（矩阵 2 数）：骨架 1777 B ≤ 1800 B、全文 1966 B ≤ 2000 B，两值同批冻结
/// （内容变更须同批更新，MUST NOT 抬锚）。
#[test]
fn 骨架与全文双门限冻结() {
    let skeleton_bytes = lf_bytes(skeleton().trim_end());
    assert_eq!(
        skeleton_bytes, 1777,
        "骨架体量实测 1777 B（LF 归一，五段骨架 + 三资产分工）已冻结；内容变更须同批更新\
         本值与全文锚"
    );
    assert!(
        skeleton_bytes <= 1800,
        "骨架体量 {skeleton_bytes} B 超过门限 1800 B"
    );

    let full_bytes = lf_bytes(&text());
    assert_eq!(
        full_bytes, 1966,
        "全文体量实测 1966 B（LF 归一，派生段含两条资产路径）已冻结；内容变更须同批更新\
         本值与骨架锚"
    );
    assert!(
        full_bytes <= 2000,
        "全文体量 {full_bytes} B 超过门限 2000 B"
    );
}

/// 边界（矩阵 2 形态）：`knowledge()` 以 `skeleton().trim_end()` 起手，派生段是追加；
/// 两处派生目录（模型声明目录 / 契约资产目录）逐字在场，段内零索引段。
#[test]
fn 派生段零改动() {
    let full = text();
    let head = skeleton().trim_end();
    assert!(full.starts_with(head), "全文以骨架 trim_end 起手");
    assert!(full.len() > head.len(), "派生段确为追加");

    let suffix = &full[head.len()..];
    assert!(
        suffix.contains(&format!(
            "模型目录：{}",
            foundation::layout::archi_graph_model_rel_dir()
        )),
        "模型声明目录经 layout 派生：{suffix}"
    );
    assert!(
        suffix.contains(&format!(
            "契约资产：{}/<分层目录…>/<叶段>.md",
            foundation::layout::archi_models_rel_dir()
        )),
        "契约资产目录经 layout 派生：{suffix}"
    );
    assert!(
        !suffix.contains("INDEX.toml"),
        "派生段零索引路径（索引经架构页视图，不进知识文本）"
    );
}

// ---------------------------------------------------------------------------
// 指路面
// ---------------------------------------------------------------------------

/// 边界：骨架出现的每个 `archi_*` 词都在 `mount::registry()` 命中（零指向不存在工具）。
#[test]
fn 指路不指向不存在工具() {
    let source = skeleton();
    let names: Vec<&str> = crate::sdk::mount::registry()
        .iter()
        .map(|mount| mount.definition.name.as_str())
        .collect();
    let mut mentioned: Vec<String> = Vec::new();
    for token in source.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')) {
        if token.starts_with("archi_") && token.len() > "archi_".len() {
            mentioned.push(token.to_owned());
        }
    }
    mentioned.sort();
    mentioned.dedup();
    assert!(!mentioned.is_empty(), "骨架应点名若干 archi 工具");
    for tool in mentioned {
        assert!(
            names.contains(&tool.as_str()),
            "骨架点名的 `{tool}` 必须在挂载清单内（零幽灵指路）"
        );
    }
    assert!(
        names.contains(&"archi_element") && names.contains(&"archi_relation"),
        "两写工具在挂载清单内（骨架的工具面与清单同源）"
    );
}

/// 表驱动（收窄后的行集）：骨架只留不可脚本化的判断，字段形态归工具面——`api` /
/// `kind` / `path` / `composite` / `archi_api` / `config.json` / `file:line` /
/// `MUST NOT 手改` / 空 schema 形态 / `x-` 面封闭逐条在场；`entry` / `doc` / 字段序 /
/// `orphanHierarchy` 一律离场（工具面字段清单持有）。
#[test]
fn 指令表收窄且新行集可判() {
    let source = skeleton();
    for needle in [
        "`api`",
        "`kind`",
        "`path`",
        "`composite`",
        "`archi_api`",
        "`config.json`",
        "`file:line`",
        "MUST NOT 手改",
        "`$defs {}`",
        "`x-function`",
    ] {
        assert!(source.contains(needle), "收窄后的行集含 `{needle}`");
    }
    for retired in ["orphanHierarchy", "`entry`", "`doc`", "字段序"] {
        assert!(
            !source.contains(retired),
            "旧指令行离场（`{retired}` 归工具面字段清单，骨架不列）"
        );
    }
}

// ---------------------------------------------------------------------------
// 旧立场零残留
// ---------------------------------------------------------------------------

#[test]
fn 知识面零旧立场() {
    let source = skeleton();
    for stale in stale_stances() {
        assert!(
            !source.contains(stale.as_str()),
            "知识面零旧立场句 `{stale}`"
        );
    }
    assert!(
        source.contains("写两件：`archi_element` / `archi_relation`"),
        "知识面以两写工具为写面表述"
    );
    assert!(
        source.contains("`archi_api` 三动作：`bootstrap` 草稿 / `link` 算 `$ref` / `check` 两轴"),
        "`archi_api` 三动作逐条在案"
    );
}

/// 边界：骨架零 `<>` 占位符 / 零模板插值（占位符即「路径未派生」的形态，由
/// `build` 的 `Err` 兜底）。
#[test]
fn 骨架零尖括号占位() {
    let source = skeleton();
    for stale in ["<>", "<FQN>", "{{"] {
        assert!(!source.contains(stale), "骨架零占位形态 `{stale}`");
    }
    let error =
        crate::archi_knowledge::build("# 标题\n\n路径 <> 未派生\n").expect_err("占位符应 Err");
    assert!(error.contains("占位符"), "文案含占位符：{error}");
}

// ---------------------------------------------------------------------------
// 模块 doc 与拼装语义零回归
// ---------------------------------------------------------------------------

/// 正向：模块头 doc 段落描述随动——三资产分工 / 设计顺序 / 三条不可越界 / 工具面 /
/// 留给判断 + 派生路径段逐段在场，「六段」零命中。
#[test]
fn 头注释段落随动() {
    let source = include_str!("archi_knowledge.rs");
    for piece in [
        "三资产分工",
        "设计顺序",
        "三条不可越界",
        "工具面",
        "留给判断",
        "派生路径段",
    ] {
        assert!(source.contains(piece), "模块 doc 含 `{piece}`");
    }
    assert!(!source.contains("六段"), "旧「六段」表述零命中");
    assert!(
        source.contains("include_str!(\"archi-knowledge.md\")"),
        "骨架经 `include_str!` 内嵌（编译期静态资产）"
    );
    assert!(
        source.contains("尽力增强、不炸 run"),
        "降级纪律一条在案（降级语义不变）"
    );
}

/// 异常（既有维持）：`build` 对空白骨架 `Err`（文案「知识骨架为空」）、`knowledge()`
/// 不降级、真实骨架零 `<` 之外的占位形态。
#[test]
fn 拼装与降级语义不变() {
    for skeleton in ["", "   \n\t\n"] {
        let error = crate::archi_knowledge::build(skeleton).expect_err("空骨架应 Err");
        assert!(error.contains("知识骨架为空"), "文案含成因：{error}");
    }
    assert!(
        crate::archi_knowledge::knowledge().is_some(),
        "真实骨架不降级"
    );
    assert!(
        text().contains("模型目录：") && text().contains("契约资产："),
        "全文恒含派生两段"
    );
}

/// 边界（既有维持）：拼装前缀不变——以骨架 `trim_end` 为前缀、派生段为纯追加
/// （`full.len() > head.len()`），且骨架首尾分别以标题起、以留白结束。
#[test]
fn 拼装前缀不变() {
    let full = text();
    let head = skeleton().trim_end();
    assert_eq!(&full[..head.len()], head, "前缀逐字等于骨架 trim_end");
    assert!(full.len() > head.len(), "派生段为追加");
    assert!(
        skeleton().trim_start().starts_with("# 架构模型须知"),
        "骨架以标题起手"
    );
    assert!(
        !skeleton().trim_end().ends_with(char::is_whitespace),
        "骨架尾部无残留空白"
    );
}

/// 异常（立场换面）：旧立场三句零命中（「契约 md 是唯一真相」/「四类生成物」/
/// 「api-* fence」），新立场「schema 是唯一凭据」在场——真相持有者换面而纪律不变。
#[test]
fn 立场换面() {
    let source = skeleton();
    for stale in [
        ["契约 md 是", "唯一真相"].concat(),
        ["四类", "生成物"].concat(),
        ["api-", "* fence"].concat(),
    ] {
        assert!(
            !source.contains(stale.as_str()),
            "知识面零旧立场句 `{stale}`"
        );
    }
    assert!(
        source.contains("schema（唯一凭据）"),
        "新立场「schema 是唯一凭据」在场"
    );
    assert!(source.contains("md 答理由"), "文档侧表述为「答理由」");
}
