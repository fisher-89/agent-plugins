//! 工具挂载单源：一张挂载清单（定义 / 准入档 / 执行体三元组）是工具面的唯一
//! 事实源。成员校验、tier 准入、`RunStarted.tools`、loop 执行分派四处全查同一
//! `definitions` 产出——MUST NOT 各自维护第二份名单。
//!
//! `finish_step` 与 `AskUserQuestion` MUST NOT 进静态清单：两者均由 [`definitions`]
//! 按注入面的 `finish` / `ask` 位铸造后追加（前者的 evaluator 形态 ID enum 取自
//! 本轮 prompt，后者按会话档位只换执行体）。
//!
//! provider 侧不变量：原生 ask 工具（provider 自己下发的同名工具）一律不挂——本
//! 清单的 ask 项即全量覆盖，MUST NOT 同时下挂 provider 原生同名工具。

use std::sync::OnceLock;

use archi::dto::{ElementKind, GateTier, WarningKind};
use rig::completion::ToolDefinition;

/// 七通用工具封闭集（**定义作者处**：`[`build_registry`]` 前七项与
/// [`generic_definition`] 的 match 臂由它驱动）。MUST NOT 参与任何判定——准入
/// 档与成员校验全查挂载清单。
pub(crate) const TOOL_NAMES: [&str; 7] = ["read", "grep", "glob", "ls", "write", "edit", "bash"];

/// 七内建工具名（装配臂与 `tools: None` 的底座名字清单单点——出口只做**列举**，
/// 准入档与成员判定仍全查 [`registry`]）。
pub(crate) fn builtin() -> &'static [&'static str] {
    &TOOL_NAMES
}

/// `create_explore_note` 专用工具名（explore 会话名字清单注入；执行体在
/// [`crate::sdk::explore_note`]，经 `LoopTurn` 上下文路由——不经 `tools::execute`
/// 纯函数）。
pub(crate) const EXPLORE_NOTE_TOOL: &str = "create_explore_note";

/// `archi_module` 专用只读工具名（定位：已知在看哪个模块）。执行体在
/// [`crate::sdk::archi_tool`]，下发面单源在注入矩阵。
pub(crate) const ARCHI_MODULE_TOOL: &str = "archi_module";

/// `archi_search` 专用只读工具名（发现：还不知道功能归谁管）。执行体同上。
pub(crate) const ARCHI_SEARCH_TOOL: &str = "archi_search";

/// `archi_api` 专用只读工具名（契约面自检：`check` 两轴 / `bootstrap` 草稿）。执行体
/// 同上；**只读零写盘**——生成物写盘单点对 agent 面不可达（`archi-runtime` 不被
/// `agent-runtime` 依赖），重生成仍由会话经既有 `bash` 命令完成。
pub(crate) const ARCHI_API_TOOL: &str = "archi_api";

/// 挂载项的准入档：`Protocol` 与 `Read` 三档恒放行，`Write` / `Execute` 需非
/// Default 档（与改造前三张按名常量表逐格等价）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolTier {
    /// 只读面（read / grep / glob / ls / archi_module / archi_search / archi_api）
    Read,
    /// 写面（write / edit / create_explore_note）
    Write,
    /// 执行面（bash）
    Execute,
    /// 协议面：任何档位放行（`finish_step`）
    Protocol,
}

/// 挂载项的执行体分类（loop 分派单点，取代按工具名的 `if` 链）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolExec {
    /// 七通用工具：`tools::execute` 纯函数
    Pure,
    /// `create_explore_note`：`LoopTurn.explore_note` 上下文
    ExploreNote,
    /// 三个 archi 只读工具：`LoopTurn.archi` 上下文
    Archi,
    /// 收口工具：编排侧校验 + 本步收口（`orchestration::finish`）
    FinishStep(agent::FinishSpec),
    /// ask 工具：一个变体带档位（schema 单源，仅执行体行为不同）
    Ask(agent::AskMode),
}

/// 挂载项三元组（定义 / 准入档 / 执行体）。不含 `PartialEq`——`ToolDefinition`
/// 内含 `serde_json::Value`，无 `Eq`（本类型驻 infra，不承担 `Eq` 链）。
#[derive(Debug, Clone)]
pub(crate) struct ToolMount {
    pub(crate) definition: ToolDefinition,
    pub(crate) tier: ToolTier,
    pub(crate) exec: ToolExec,
}

/// 静态挂载清单（`OnceLock` 惰性单点：`ToolDefinition` 三字段均非 `const` 可
/// 构造）。
static REGISTRY: OnceLock<Vec<ToolMount>> = OnceLock::new();

/// 十一项静态挂载清单（唯一工具事实源）。**前七项必须恰是七内建**——`tools: None`
/// 的语义载体（[`definitions`] 直接取前七项）。
pub(crate) fn registry() -> &'static [ToolMount] {
    REGISTRY.get_or_init(build_registry)
}

/// 十一个字面构造项按序：七内建（Read ×4 / Write ×2 / Execute ×1 + `Pure`）→
/// `create_explore_note`（Write / `ExploreNote`）→ 三 archi 只读（Read / `Archi`）。
fn build_registry() -> Vec<ToolMount> {
    let mut mounts: Vec<ToolMount> = Vec::with_capacity(11);
    for name in TOOL_NAMES {
        mounts.push(ToolMount {
            definition: generic_definition(name),
            tier: match name {
                "write" | "edit" => ToolTier::Write,
                "bash" => ToolTier::Execute,
                _ => ToolTier::Read,
            },
            exec: ToolExec::Pure,
        });
    }
    mounts.push(ToolMount {
        definition: explore_note_definition(),
        tier: ToolTier::Write,
        exec: ToolExec::ExploreNote,
    });
    mounts.push(ToolMount {
        definition: archi_module_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Archi,
    });
    mounts.push(ToolMount {
        definition: archi_search_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Archi,
    });
    mounts.push(ToolMount {
        definition: archi_api_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Archi,
    });
    mounts
}

/// 按名取定义（克隆出口）：compose 装配层成员校验（`None` 即非法名）与
/// [`definitions`] 取表的共同单点。
pub(crate) fn lookup(name: &str) -> Option<ToolDefinition> {
    registry()
        .iter()
        .find(|mount| mount.definition.name == name)
        .map(|mount| mount.definition.clone())
}

/// 本轮下发的挂载清单：`tools = None` → 内建七件；`Some(names)` → 按清单序
/// 过滤（未知名 `Err`，文案携非法名）；`ask` 位置位时追加 `AskUserQuestion`，
/// `finish` 位置位时追加 `finish_step`（两条独立分支，序为 ask 先、finish 后）。
pub(crate) fn definitions(
    injections: &agent::SessionInjections,
    prompt: &str,
) -> Result<Vec<ToolMount>, String> {
    let mut mounts: Vec<ToolMount> = match &injections.tools {
        None => registry().iter().take(TOOL_NAMES.len()).cloned().collect(),
        Some(names) => {
            for name in names {
                if lookup(name).is_none() {
                    return Err(format!("注入面非法工具名: {name}（挂载清单外）"));
                }
            }
            registry()
                .iter()
                .filter(|mount| names.iter().any(|name| name == &mount.definition.name))
                .cloned()
                .collect()
        }
    };
    // ask 位与 finish 位**互不依赖**（两条独立 `if`，无合取判据）：`finish = None`
    // 的会话仍有 ask，`ask = None` 的会话仍有 `finish_step`
    if let Some(mode) = injections.ask {
        mounts.push(ToolMount {
            definition: ToolDefinition {
                name: orchestration::ask::TOOL_NAME.to_owned(),
                description: orchestration::ask::description().to_owned(),
                parameters: orchestration::ask::parameters(),
            },
            tier: ToolTier::Protocol,
            exec: ToolExec::Ask(mode),
        });
    }
    if let Some(spec) = injections.finish {
        mounts.push(ToolMount {
            definition: ToolDefinition {
                name: orchestration::finish::TOOL_NAME.to_owned(),
                description: orchestration::finish::description(spec).to_owned(),
                parameters: orchestration::finish::parameters(spec, prompt),
            },
            tier: ToolTier::Protocol,
            exec: ToolExec::FinishStep(spec),
        });
    }
    Ok(mounts)
}

/// 词表拼接（`/` 分隔；值域清单由词汇单点派生，工具描述与响应共用同一份词表）。
fn vocabulary(words: impl IntoIterator<Item = &'static str>) -> String {
    words.into_iter().collect::<Vec<&str>>().join(" / ")
}

/// `archi_module` 定义（定位：邻接 / 直接子元素 / 文档引用 / 声明位置）。
fn archi_module_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_MODULE_TOOL.to_owned(),
        description: format!(
            "查询架构模型里的模块（定位：已知道在看哪个模块）。指定 element 返回该元素的\
             kind（{}）/ gate（{}）/ path / metadata、声明位置（modelFile + modelLine，可直接 edit 到该行）、\
             出边与入边（对端 FQN + 说明）、直接子元素与关联文档（四字段条目，正文用 read 自取）；\
             省略 element 返回全部元素的薄概览。模型写坏时返回结构化诊断（先修模型）；\
             docs 指向的文件不存在时以 {} 警告给出（只报不拦）",
            vocabulary(ElementKind::ALL.map(ElementKind::as_str)),
            vocabulary(GateTier::ALL.map(GateTier::as_str)),
            WarningKind::DocNotFound.as_str(),
        ),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "element": { "type": "string", "description": "元素 FQN（如 App.Auth）；省略时返回全部元素的薄概览" }
            },
            "required": []
        }),
    }
}

/// `archi_search` 定义（发现：还不知道功能归谁管）。
fn archi_search_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_SEARCH_TOOL.to_owned(),
        description: "在架构模型里按关键词检索元素与文档路径（发现：还不知道这功能归谁管时先搜）。\
             匹配对象为元素 FQN / summary / metadata 值 / 文档路径与文件名（不搜文档正文——\
             正文用 grep），大小写不敏感且支持驼峰分词（AuthToken → auth / token）。返回元素\
             薄条目与四字段文档条目"
            .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "关键词 / 元素名片段 / 文档路径片段（不得为空白）" },
                "scope": { "type": "string", "enum": ["elements", "docs", "all"], "description": "检索范围（缺省 all）" }
            },
            "required": ["query"]
        }),
    }
}

/// `archi_api` 定义（契约面自检：`check` 两轴 / `bootstrap` 草稿；零写盘）。
fn archi_api_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_API_TOOL.to_owned(),
        description: concat!(
            "查询模块 API 契约面的两轴事实（自检）。`check` 返回生成物新鲜度（三类差异：",
            "内容不符 / 应删未删 / 应生成未生成）与契约判据摘要（按 category 计数 + 逐条 ",
            "kind / element / message）；`bootstrap` 按元素 FQN 输出契约 md 草稿（只出 ",
            "stdout 语义、零写盘）。模型结构错时返回诊断清单（先修模型）。生成物过期用既有",
            "命令重生成（pnpm api:export / cargo run -p archi-api-gen -- generate），MUST NOT ",
            "手改生成物"
        )
        .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["check", "bootstrap"], "description": "动作（封闭二值）：check 查两轴 / bootstrap 出草稿" },
                "element": { "type": "string", "description": "元素 FQN（bootstrap 必填，如 App.Auth）" }
            },
            "required": ["action"]
        }),
    }
}

/// `create_explore_note` 定义（无路径参数——服务端按会话解析目标）。
fn explore_note_definition() -> ToolDefinition {
    ToolDefinition {
        name: EXPLORE_NOTE_TOOL.to_owned(),
        description: "为当前探索会话的笔记落盘并定名。无路径参数——服务端按当前会话解析目标：\
             占位会话（未定名）原子完成文件创建与记录定名（一次调用即最终名，勿先用 bash 造\
             文件）；已定名会话仅接受与记录同名且文件缺失的补建。返回笔记路径（repo 相对、\
             `/` 分隔），供 edit 续用"
            .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "本次探索的 kebab-case 最终名（小写字母/数字，可用 `-` 连接；已定名会话必须与记录同名）" },
                "content": { "type": "string", "description": "笔记初稿全文（原样落盘）" }
            },
            "required": ["name", "content"]
        }),
    }
}

/// 七通用工具定义（[`TOOL_NAMES`] 逐名 match；内容与参数化前的字面量逐字
/// 一致）。
fn generic_definition(name: &str) -> ToolDefinition {
    match name {
        "read" => ToolDefinition {
            name: "read".to_owned(),
            description:
                "读取文件内容（行号前缀输出）。可选 offset（1 起始行号）与 limit（行数）；\
                          单次至多 2000 行，超限截断留痕，可用 offset 翻页读取后续窗口。\
                          未指定 offset / limit 且文件较大时只预览前 100 行并提示分页——\
                          需要整读请显式给 offset / limit"
                    .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "offset": { "type": "integer", "description": "起始行号（1 起始，缺省 1）" },
                    "limit": { "type": "integer", "description": "读取行数（缺省至多 2000 行读到文件尾）" }
                },
                "required": ["path"]
            }),
        },
        "grep" => ToolDefinition {
            name: "grep".to_owned(),
            description:
                "行级正则匹配：返回命中行（文件路径:行号: 内容）；path 传目录时递归扫描其下文件；\
                 可选 context 携带匹配行上下文（命中行 ± N 行）"
                    .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件或目录路径（目录时递归扫描）" },
                    "pattern": { "type": "string", "description": "正则表达式（regex 语法，非法正则报错）" },
                    "context": { "type": "integer", "description": "匹配行上下文行数（命中行 ± N，缺省 0；窗口合并去重，不连续组间以 -- 分隔）" }
                },
                "required": ["path", "pattern"]
            }),
        },
        "glob" => ToolDefinition {
            name: "glob".to_owned(),
            description: "按 glob 模式列出 workspace 内匹配文件（root 相对模式，上限 200 条）"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "root 相对 glob 模式（如 src/**/*.rs）" }
                },
                "required": ["pattern"]
            }),
        },
        "ls" => ToolDefinition {
            name: "ls".to_owned(),
            description: "列目录：一级条目（目录带 / 后缀），字母序".to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内目录路径" }
                },
                "required": ["path"]
            }),
        },
        "write" => ToolDefinition {
            name: "write".to_owned(),
            description: "覆写文件（全量覆盖；缺失父目录自动创建）".to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "content": { "type": "string", "description": "写入全文" }
                },
                "required": ["path", "content"]
            }),
        },
        "edit" => ToolDefinition {
            name: "edit".to_owned(),
            description: "精确串替换：old_string 在文件中必须唯一命中（多处需 replace_all）"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "workspace 内文件路径" },
                    "old_string": { "type": "string", "description": "待替换的精确原文" },
                    "new_string": { "type": "string", "description": "替换后的文本" },
                    "replace_all": { "type": "boolean", "description": "全部替换（缺省仅首处且要求唯一命中）" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        "bash" => ToolDefinition {
            name: "bash".to_owned(),
            description: "执行 shell 命令并返回合并输出（stdout 段在前、stderr 段带 [stderr] \
                          标记在后）；cwd 为 workspace root，非零退出码报错并携带输出。超时为活性\
                          护栏，超时即终止进程树"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "待执行的命令行（git-bash / sh unix 语法优先，探测失败退 cmd）" },
                    "timeout_ms": { "type": "integer", "description": "超时毫秒（缺省 120000；须在 1000-1800000 内，越界报错不执行，超时终止进程树）" }
                },
                "required": ["command"]
            }),
        },
        other => unreachable!("generic_definition 仅承接 TOOL_NAMES 封闭集: {other}"),
    }
}

/// 测试夹具：按名字清单构造注入面（`finish` 缺省 `None`），压平字面构造面。
#[cfg(test)]
pub(crate) fn injected(names: &[&str]) -> agent::SessionInjections {
    injected_finish(names, None)
}

/// 测试夹具同族：名字清单 + 收口档位（`finish` 位置位的正向面）。
#[cfg(test)]
pub(crate) fn injected_finish(
    names: &[&str],
    finish: Option<agent::FinishSpec>,
) -> agent::SessionInjections {
    agent::SessionInjections {
        preamble: None,
        tools: Some(names.iter().map(|name| (*name).to_owned()).collect()),
        finish,
        ask: None,
    }
}
