//! `archi_knowledge` / `archi-knowledge.md` 的单元测试：知识面收窄后的五段结构（三资产
//! 分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断 + 兼容一句）、零语法教学（零示例
//! 围栏 / 零键形示例 / 零字段值域行 / 零 `metadata` 示例）、零结构诊断可执行词、双门限
//! 体量冻结（骨架 ≤ 1831 B / 全文 ≤ 2031 B）、两处派生目录仍在行程、指路只指向真实工具、
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

/// 边界（矩阵 2 数）：骨架 1823 B ≤ 1831 B、全文 2012 B ≤ 2031 B，两值同批重冻
/// （抬锚须显式豁免并同批更新四数）。
#[test]
fn 骨架与全文双门限冻结() {
    let skeleton_bytes = lf_bytes(skeleton().trim_end());
    assert_eq!(
        skeleton_bytes, 1823,
        "骨架体量实测 1823 B（LF 归一，换指后重冻）已冻结；抬锚须显式豁免并同批\
         更新本值与全文锚"
    );
    assert!(
        skeleton_bytes <= 1831,
        "骨架体量 {skeleton_bytes} B 超过门限 1831 B"
    );

    let full_bytes = lf_bytes(&text());
    assert_eq!(
        full_bytes, 2012,
        "全文体量实测 2012 B（LF 归一，骨架 1823 + 派生段 189）已冻结；抬锚须显式豁免并同批\
         更新本值与骨架锚"
    );
    assert!(
        full_bytes <= 2031,
        "全文体量 {full_bytes} B 超过门限 2031 B"
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
        "`x-constructor`",
        "`x-static`",
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

// ---------------------------------------------------------------------------
// 「三条不可越界」② 的 `x-` 面五项逐名（AC-10）
// ---------------------------------------------------------------------------

/// 骨架内「`x-` 面封闭为 …」那一行（五项枚举的唯一落点）。
fn x_surface_line() -> &'static str {
    skeleton()
        .lines()
        .find(|line| line.contains("`x-` 面封闭为"))
        .expect("骨架应含 `x-` 面封闭的那一行")
}

/// 正向（AC-10）：五项逐名在场（带反引号原样）、按「 / 」逐个连接；三条不可越界各至少
/// 一处；「空 schema」/「不要编造导出」/「历史 fence 读面不读、MUST NOT 新增」在案。
#[test]
fn 五项逐名在场() {
    let line = x_surface_line();
    let ordered = "`x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`";
    assert!(line.contains(ordered), "五项按名逐个连接：{line}");

    let source = skeleton();
    for needle in [
        "文档不列接口",
        "schema 不写理由",
        "TOML 不造空元素",
        "空 schema",
        "不要编造导出",
        "历史 fence 读面不读、MUST NOT 新增",
    ] {
        assert!(source.contains(needle), "骨架含 `{needle}`");
    }
}

/// 边界（枚举形态）：五项零缩写 / 零「等」/ 零「见 spec」类指路；不带反引号的裸词不计入
/// （needle 带反引号原样）；`x-` 面以外零新协议词。
#[test]
fn 五项枚举形态可判() {
    let line = x_surface_line();
    assert!(line.contains("封闭为"), "枚举以「封闭为」起手：{line}");
    for forbidden in ["见 spec", "等", "…", "etc"] {
        assert!(
            !line.contains(forbidden),
            "枚举零指路 `{forbidden}`：{line}"
        );
    }
    for name in [
        "x-function",
        "x-class",
        "x-common",
        "x-constructor",
        "x-static",
    ] {
        assert!(
            line.contains(&format!("`{name}`")),
            "带反引号原样（裸词不计入）：缺 `{name}`"
        );
    }
    assert_eq!(
        line.matches(" / ").count(),
        4,
        "五项之间恰四个分隔符：{line}"
    );

    let source = skeleton();
    for stray in ["x-interface", "x-nominal", "x-singleSource"] {
        assert!(!source.contains(stray), "`x-` 面以外零新协议词 `{stray}`");
    }
}

/// 边界（增量口径）：全文增量 == 骨架增量——派生段体量固定（189 B），内容变更全在骨架；
/// 全文以骨架 `trim_end()` 起手、派生段为零追加。
#[test]
fn 增量全在骨架() {
    let full = text();
    let head = skeleton().trim_end();
    assert!(full.starts_with(head), "全文以骨架 `trim_end` 起手");

    let suffix = &full[head.len()..];
    let suffix_bytes = lf_bytes(suffix);
    assert_eq!(
        suffix_bytes, 189,
        "派生段体量 189 B 已冻结（两条 layout 目录 + 固定文案）；骨架的任何增量都使全文等量增长"
    );
    assert_eq!(
        lf_bytes(&full) - lf_bytes(head),
        189,
        "全文增量 == 骨架增量（增量全在骨架）"
    );
}

/// 异常（判据面事实不进骨架）：零「构造通路」/「object 等同 JSON 对象」类表述、零
/// `<>` 占位符、零示例围栏 / 零 `[element.` 前缀。
#[test]
fn 判据面事实不进骨架() {
    let source = skeleton();
    for forbidden in ["构造通路", "object 等同 JSON 对象"] {
        assert!(
            !source.contains(forbidden),
            "判据面事实不进骨架 `{forbidden}`"
        );
    }
    assert_eq!(source.matches("```").count(), 0, "零示例围栏");
    assert!(!source.contains("[element."), "零 `[element.` 前缀");
    assert!(!source.contains("<>"), "零占位符形态");
}

/// 回归（体量与文案随动）：测试模块头写「骨架 ≤ 1831 B / 全文 ≤ 2031 B」，测试 doc 写
/// 「抬锚须显式豁免并同批更新四数」；两处 `assert_eq!` 失败文案写死重冻后的实测值（1823 /
/// 2012）；退役阈值与旧实测值零残留（产品模块零旧门限）。
#[test]
fn 体量门限文案随动() {
    let tests = include_str!("archi_knowledge_test.rs");
    assert!(
        tests.contains("≤ 1831 B") && tests.contains("≤ 2031 B"),
        "测试模块头写新门限"
    );
    assert!(
        tests.contains("抬锚须显式豁免并同批更新四数"),
        "测试 doc 写抬锚纪律"
    );
    assert!(
        tests.contains("1823") && tests.contains("2012"),
        "两条 assert_eq! 失败文案写死重冻后的实测值"
    );

    let retired = [
        ["≤ ", "1800 B"].concat(),
        ["1", "777"].concat(),
        ["19", "66"].concat(),
        ["1", "808"].concat(),
        ["1", "997"].concat(),
    ];
    for phrase in retired {
        assert!(
            !tests.contains(&phrase),
            "退役阈值 / 旧实测值零残留：`{phrase}`"
        );
    }
    let product = include_str!("archi_knowledge.rs");
    for phrase in [
        ["1800", " B"].concat(),
        ["2000", " B"].concat(),
        ["18", "31"].concat(),
        ["20", "31"].concat(),
    ] {
        assert!(!product.contains(&phrase), "产品模块零旧门限：`{phrase}`");
    }
}

// ---------------------------------------------------------------------------
// 本变更：生成物指路换句（`archi_generate` 取代死指代）
// ---------------------------------------------------------------------------

/// 正向（AC-9）：骨架含新串「生成物过期调 `archi_generate` 重生成」与「MUST NOT 手改生成物」；
/// 该句与「读五件 / 写两件」行同段（位次不变）。
#[test]
fn 生成物指路换句在场且位次不变() {
    let source = skeleton();
    assert!(
        source.contains("生成物过期调 `archi_generate` 重生成"),
        "换指后的新串在场：{source}"
    );
    assert!(
        source.contains("MUST NOT 手改生成物"),
        "手改禁令同句在场：{source}"
    );

    let paragraph = source
        .split("\n\n")
        .find(|paragraph| paragraph.contains("生成物过期调"))
        .unwrap_or_else(|| panic!("换指句未落在任何段落：{source}"));
    assert!(
        paragraph.contains("读五件") && paragraph.contains("写两件"),
        "与工具面「读五件 / 写两件」同段（位次不变）：{paragraph}"
    );
}

/// 边界：旧串零残留——needle「生成物过期用既有命令重生成」与「既有命令」零命中；句内的
/// `archi_generate` 是 `mount::registry()` 在册名（指路不指向不存在工具）。
#[test]
fn 生成物指路旧串零残留且在册() {
    let source = skeleton();
    let stale = ["生成物过期用", "既有命令", "重生成、MUST NOT 手改；"].join("");
    assert!(!source.contains(&stale), "旧整串零命中：`{stale}`");
    assert!(!source.contains("既有命令"), "旧死指代「既有命令」零命中");

    assert!(
        crate::sdk::mount::registry()
            .iter()
            .any(|mount| mount.definition.name == "archi_generate"),
        "换指的目标名在册（指路不指向不存在工具）"
    );
}

/// 异常（反向）：骨架零「本仓的生成命令」死指代、零路径模板；`.schema.json` 仍被写作手写
/// 真相（换句不得把契约说成生成物）。
#[test]
fn 生成物指路零死指代且契约不被说成生成物() {
    let source = skeleton();
    for stale in ["本仓的生成命令", "本仓", "生成命令"] {
        assert!(!source.contains(stale), "骨架零死指代 `{stale}`：{source}");
    }
    for template in [".dev-team", "architecture/models", "<>"] {
        assert!(
            !source.contains(template),
            "骨架零路径模板 `{template}`（路径由派生段追加）"
        );
    }
    for sentence in source
        .split(['。', '\n'])
        .filter(|sentence| sentence.contains(".schema.json"))
    {
        assert!(
            ["唯一凭据", "write", "edit", "手写"]
                .iter()
                .any(|piece| sentence.contains(piece)),
            "`.schema.json` 所在句须写其手写来源：{sentence}"
        );
        assert!(
            !sentence.contains("生成物"),
            "`.schema.json` 不得被说成生成物：{sentence}"
        );
    }
}

// ---------------------------------------------------------------------------
// 本变更：体量冻结重冻（门限不抬，实测值随句改写）
// ---------------------------------------------------------------------------

/// 正向：骨架字节数按 LF 归一后 `<= 1831`、全文 `<= 2031`；两处**精确断言**按实测值重冻
/// （门限**不抬**）：实测 1823 / 2012 与 `design.md 附录 D` 的字节账一致。
#[test]
fn 体量冻结重冻实测值与门限一致() {
    let skeleton_bytes = lf_bytes(skeleton().trim_end());
    let full_bytes = lf_bytes(&text());
    assert_eq!(skeleton_bytes, 1823, "骨架实测值重冻为 1823 B");
    assert_eq!(full_bytes, 2012, "全文实测值重冻为 2012 B");
    assert_eq!(
        full_bytes - skeleton_bytes,
        189,
        "增量全在骨架之外（派生段体量固定 189 B）"
    );
    assert!(skeleton_bytes <= 1831, "骨架仍在门限内（不抬门限）");
    assert!(full_bytes <= 2031, "全文仍在门限内（不抬门限）");
}

/// 边界：派生段仍为追加（`full[..head.len()] == head.trim_end()` 且 `full.len() > head.len()`）；
/// CRLF 检出不得改变冻结值（`lf_bytes` 归一）。
#[test]
fn 体量冻结派生段追加且行尾归一() {
    let full = text();
    let head = skeleton().trim_end();
    assert!(full.starts_with(head), "全文以骨架 trim_end 起手");
    assert!(full.len() > head.len(), "派生段确为追加");
    assert_eq!(lf_bytes(&full), 2012, "全文按 LF 归一的体量与冻结值一致");

    // CRLF 检出等价：同一文本的 CRLF 形态归一后与 LF 形态同值
    let crlf = skeleton().replace('\n', "\r\n");
    assert_eq!(
        lf_bytes(&crlf),
        lf_bytes(skeleton()),
        "CRLF 检出不得改变冻结值"
    );
}

/// 异常：超限时断言文案如实报「超过门限 1831 B / 2031 B」（重冻失败即红，不静默放宽）；零
/// 其它字节改动的可观测面：五段小节标题与三资产分工三句逐字仍在。
#[test]
fn 体量冻结超限文案如实且结构未动() {
    let tests = include_str!("archi_knowledge_test.rs");
    assert!(
        tests.contains("超过门限 1831 B") && tests.contains("超过门限 2031 B"),
        "超限断言文案如实报门限值"
    );
    assert!(
        tests.contains("骨架体量实测 1823 B") && tests.contains("全文体量实测 2012 B"),
        "重冻实测值写进失败文案"
    );

    let source = skeleton();
    for heading in [
        "# 架构模型须知",
        "## 三资产分工",
        "## 设计顺序",
        "## 三条不可越界",
        "## 工具面",
        "## 留给判断",
    ] {
        assert!(source.contains(heading), "五段标题仍在：`{heading}`");
    }
    for piece in ["TOML 答拓扑", "schema（唯一凭据）答接触面", "md 答理由"] {
        assert!(source.contains(piece), "三资产分工句仍在：`{piece}`");
    }
}

// ---------------------------------------------------------------------------
// 本变更：`archi_knowledge.rs` 模块头措辞随动（三写工具口径 + 门限只在测试侧）
// ---------------------------------------------------------------------------

/// 模块头正文（`//!` 段）：去 `//!` 前缀与行首尾空白后逐行拼接——中文散文在源码里按
/// 显示宽度折行，「只写三类生成物」一类短语会被行尾切断，故判据落在**去前缀后的连续
/// 正文**上，而不是原始行。
fn module_head(source: &str) -> String {
    source
        .lines()
        .take_while(|line| line.trim_start().starts_with("//!"))
        .map(|line| line.trim_start().trim_start_matches("//!").trim())
        .collect::<Vec<_>>()
        .join("")
}

/// 正向（AC-9）：模块头含生成物写工具的说明——工具面共三写（两模型写 + 一生成物写），第三
/// 个只写三类生成物、不写模型与契约、不占「写面名单」；既有「写两件」是**模型**写面的口径
/// 仍在（两者不混同）。
#[test]
fn 模块头三写口径随动() {
    let source = include_str!("archi_knowledge.rs");
    let head = module_head(source);
    assert!(!head.is_empty(), "模块头（`//!` 段）在案：{source}");
    for piece in [
        "写两件",
        "三写工具（两模型写 + 一生成物写）",
        "第三个写工具",
        "archi_generate",
        "只写三类生成物",
        "不占写面名单",
    ] {
        assert!(head.contains(piece), "模块头含 `{piece}`：{head}");
    }
    assert!(
        !source.contains("两写工具"),
        "旧口径「两写工具」零命中（已改三写口径）"
    );
}

/// 边界（D-16）：该文件内零数字门限（门限只冻结在测试侧）——`1831` / `2031` 与旧门限
/// 字面在本文件零命中。
#[test]
fn 模块头零数字门限() {
    let source = include_str!("archi_knowledge.rs");
    for threshold in [
        ["18", "31"].concat(),
        ["20", "31"].concat(),
        ["18", "00 B"].concat(),
        ["20", "00 B"].concat(),
    ] {
        assert!(
            !source.contains(&threshold),
            "产品模块零数字门限 `{threshold}`（门限只冻结在测试侧）"
        );
    }
}

/// 异常（反向）：知识「指路不指向不存在工具」的机制句仍表述为可核对形态（工具面归工具
/// 定义、路径归代码派生），零新增能力宣称；`knowledge()` 零 `std::fs` / 零环境变量读取。
#[test]
fn 模块头机制句未被改写为承诺() {
    let source = include_str!("archi_knowledge.rs");
    for mechanism in ["从代码派生", "尽力增强、不炸 run", "字段形态"] {
        assert!(source.contains(mechanism), "机制句仍在：`{mechanism}`");
    }
    for claim in ["自动生成", "保证一致", "永不漂移"] {
        assert!(
            !source.contains(claim),
            "零新增能力宣称 `{claim}`：{source}"
        );
    }
    assert!(
        !source.contains("std::fs"),
        "拼装面零文件 IO（`include_str!` 是编译期内嵌）"
    );
    assert!(!source.contains("std::env"), "拼装面零环境变量读取");
}
