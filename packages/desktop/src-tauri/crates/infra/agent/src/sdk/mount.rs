//! 工具挂载单源：一张挂载清单（定义 / 准入档 / 执行体三元组）是工具面的唯一
//! 事实源。成员校验、tier 准入、`RunStarted.tools`、loop 执行分派四处全查同一
//! `definitions` 产出——MUST NOT 各自维护第二份名单。
//!
//! `finish_step` 与 `ask_user_question` MUST NOT 进静态清单：两者均由 [`definitions`]
//! 按注入面的 `finish` / `ask` 位铸造后追加（前者的 evaluator 形态 ID enum 取自
//! 本轮 prompt，后者按会话档位只换执行体）。
//!
//! provider 侧不变量：原生 ask 工具（provider 自己下发的同名工具）一律不挂——本
//! 清单的 ask 项即全量覆盖，MUST NOT 同时下挂 provider 原生同名工具。

use std::sync::OnceLock;

use archi::dto::{ElementKind, GateTier};
use rig::completion::ToolDefinition;

use crate::sdk::archi_drift_view::DRIFT_SCOPES;

/// 七通用工具封闭集（**定义作者处**：`[`build_registry`]` 前七项与
/// [`generic_definition`] 的 match 臂由它驱动）。MUST NOT 参与任何判定——准入
/// 档与成员校验全查挂载清单。
pub(crate) const TOOL_NAMES: [&str; 7] = ["read", "grep", "glob", "ls", "write", "edit", "bash"];

/// 七内建工具名（装配臂与 `tools: None` 的底座名字清单单点——出口只做**列举**，
/// 准入档与成员判定仍全查 [`registry`]）。
pub(crate) fn builtin() -> &'static [&'static str] {
    &TOOL_NAMES
}

/// `db_tables` 只读发现工具名（表清单 / 单表字段说明）：执行体在
/// [`crate::sdk::db_tool`]，会话可见性单源在 `compose`（explore / change 两臂）。
pub(crate) const DB_TABLES_TOOL: &str = "db_tables";

/// `db_query` 只读取数工具名（条件 / 排序 / 投影 / 分页）：执行体同上；查询语义
/// 单源在 `store`（本清单只管定义与准入档）。
pub(crate) const DB_QUERY_TOOL: &str = "db_query";

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

/// `archi_drift` 专用只读工具名（漂移判据读数：写完模型 / 改完代码后自查覆盖、归属
/// 与契约面）。执行体同上；**恒全量、零写盘**——报告落盘者恒为门禁 runner。
pub(crate) const ARCHI_DRIFT_TOOL: &str = "archi_drift";

/// `archi_scaffold` 专用只读工具名（新元素 TOML 模板：字段序 / 内联 `metadata` /
/// `path` 必填提示 / 关系模板）。执行体同上；**只读零写盘**——写面仍是既有 `write` /
/// `edit`，故本工具不在「专用写工具」之列。
pub(crate) const ARCHI_SCAFFOLD_TOOL: &str = "archi_scaffold";

/// `propose_config` 专用工具名（config 会话唯一提案出口）：常量本体驻执行体模块
/// [`crate::sdk::config_propose`]，本清单再导出供装配臂 / 沙箱白名单引用。
pub(crate) use crate::sdk::config_propose::PROPOSE_CONFIG_TOOL;

/// 挂载项的准入档：`Protocol` 与 `Read` 三档恒放行，`Write` / `Execute` 需非
/// Default 档（与改造前三张按名常量表逐格等价）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolTier {
    /// 只读面（read / grep / glob / ls / archi_module / archi_search / archi_api /
    /// archi_drift / archi_scaffold / propose_config）
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
    /// 五个 archi 只读工具：`LoopTurn.archi` 上下文
    Archi,
    /// 两个 DB 只读工具：`LoopTurn.db` 上下文（两工具共用一个执行体，工具名
    /// 由 loop 侧传入——与 `Archi` 同形）
    Db,
    /// `propose_config`：`LoopTurn.config` 上下文
    Config,
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

/// 十六项静态挂载清单（唯一工具事实源）。**前七项仍是七内建**——`tools: None`
/// 的语义载体（[`definitions`] 直接取前七项）。
pub(crate) fn registry() -> &'static [ToolMount] {
    REGISTRY.get_or_init(build_registry)
}

/// 十六个字面构造项按序：七内建（Read ×4 / Write ×2 / Execute ×1 + `Pure`）→
/// `create_explore_note`（Write / `ExploreNote`）→ 五 archi 只读（Read / `Archi`）
/// → 两 DB 只读（Read / `Db`，定义见 [`db_tables_definition`] /
/// [`db_query_definition`]）→ `propose_config`（Read / `Config`，定义见
/// [`crate::sdk::config_propose::definition`]）。
fn build_registry() -> Vec<ToolMount> {
    let mut mounts: Vec<ToolMount> = Vec::with_capacity(16);
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
    mounts.push(ToolMount {
        definition: archi_drift_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Archi,
    });
    mounts.push(ToolMount {
        definition: archi_scaffold_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Archi,
    });
    mounts.push(ToolMount {
        definition: db_tables_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Db,
    });
    mounts.push(ToolMount {
        definition: db_query_definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Db,
    });
    mounts.push(ToolMount {
        definition: crate::sdk::config_propose::definition(),
        tier: ToolTier::Read,
        exec: ToolExec::Config,
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
/// 过滤（未知名 `Err`，文案携非法名）；`ask` 位置位时追加 `ask_user_question`，
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

/// `archi_module` 定义（定位：邻接 / 直接子元素 / 入口文档 / 声明位置）。
fn archi_module_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_MODULE_TOOL.to_owned(),
        description: format!(
            "查询架构模型里的模块（定位：已知道在看哪个模块）。指定 element 返回该元素的\
             kind（{}）/ gate（{}）/ path / metadata、声明位置（modelFile + modelLine，可直接 edit 到该行）、\
             出边与入边（对端 FQN + 说明）、直接子元素与入口文档单值 doc（仅 api = none 元素有值；declared 走 api.contractPath，正文用 read 自取）；\
             省略 element 返回全部元素的薄概览，每条含 hasPath（缺 path 的元素一次判出）；\
             单元素响应另给 resolvedEntry（入口解析结果：显式命中 / 约定默认命中 / 不可用成因）；\
             新元素的字段形态用 archi_scaffold 取模板；\
             `api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）。\
             模型写坏时返回结构化诊断（先修模型）",
            vocabulary(ElementKind::ALL.map(ElementKind::as_str)),
            vocabulary(GateTier::ALL.map(GateTier::as_str)),
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
             薄条目与入口文档路径"
            .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "关键词 / 元素名片段 / 文档路径片段（不得为空白）" },
                "scope": { "type": "string", "enum": ["elements", "doc", "all"], "description": "检索范围（缺省 all）" }
            },
            "required": ["query"]
        }),
    }
}

/// `archi_api` 定义（契约面自检与草稿：`check` 两轴 / `bootstrap` 草稿；零写盘）。
fn archi_api_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_API_TOOL.to_owned(),
        description: concat!(
            "查询模块 API 契约面的两轴事实（自检与草稿）。`check` 返回生成物新鲜度（三类差异：",
            "内容不符 / 应删未删 / 应生成未生成）与契约判据摘要（按 category 计数 + 逐条 ",
            "kind / element / message）；`bootstrap` 按元素 FQN 输出契约 md 草稿——",
            "新建 / 补契约的第一步就取它（只出 stdout 语义、零写盘）。",
            "生成物四类：契约 md 同目录的 `<FQN>.schema.json`、架构域 `INDEX.toml`、",
            "入口同级 `.i.rs` / `.i.ts`——过期用既有生成命令重生成，MUST NOT 手改生成物。",
            "`check` 的 `artifacts.enabled = false` 即契约面未启用（此时 `findings` 恒空，",
            "不等于零问题）。模型结构错时返回诊断清单（先修模型）"
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

/// `archi_drift` 定义（漂移判据读数：写完模型 / 改完代码后自查覆盖、归属与契约面）。
///
/// 描述里的两句是**读数口径**而非免责声明：`gate` 全 `off` 的模型 `unmatchedFiles`
/// 是数百条量级的常态（直出即 15–25 KB 噪声），故明细只在 `scope = files` 出现。
fn archi_drift_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_DRIFT_TOOL.to_owned(),
        description: format!(
            "读架构漂移判据（写完模型 / 改完代码后自查覆盖、归属与契约面）。恒全量、零写盘。\
             gate 全 off 时 unmatchedFiles 是数百条量级的常态不是错误（明细只在 scope = files 出现，\
             摘要里只有计数位）。errors / warnings 通常为空，非空即逐条修；files 只在排查归属问题时用。\
             scope 三值：{}（缺省 summary）。每次调用都重新全量判定、零跨调用缓存——durationMs 让\
             重复调用的浪费可见",
            DRIFT_SCOPES.join(" / "),
        ),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "scope": { "type": "string", "enum": DRIFT_SCOPES, "description": "'summary'（缺省）| 'entries' | 'files'" }
            },
            "required": []
        }),
    }
}

/// `archi_scaffold` 定义（新元素模板：字段序 + 内联 `metadata` + `path` 必填提示 +
/// 关系模板；零写盘）。
///
/// `kind` 的 enum 由元素种类词表派生（词汇单点，零第二份字面量）；草稿本身
/// 过 `TomlParser` 零诊断——这是「示例 TOML 从知识骨架删除」的自证锚。
fn archi_scaffold_definition() -> ToolDefinition {
    ToolDefinition {
        name: ARCHI_SCAFFOLD_TOOL.to_owned(),
        description: concat!(
            "取新元素 TOML 模板（字段序 / 内联 metadata / path 必填提示 / 关系模板），",
            "只读零写盘——写面仍是既有 write / edit。kind 省略取 domain，值集见 enum。",
            "草稿可直接抄改（单段占位 FQN，多段时先声明祖先），改完用 archi_drift 自查",
        )
        .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "enum": ElementKind::ALL.map(ElementKind::as_str), "description": "元素 kind 四值之一（省略取 domain）" }
            },
            "required": []
        }),
    }
}

/// `db_tables` 定义（发现：先知道有哪些表、每张表是什么、字段叫什么）。
fn db_tables_definition() -> ToolDefinition {
    ToolDefinition {
        name: DB_TABLES_TOOL.to_owned(),
        description: "查询本机数据的表结构（**先 db_tables 后 db_query**）。省略 table 返回表清单，\
             每行含 dimension（user / workspace）/ table（db_query 的 table 入参取值）/ count（记录数）/ \
             summary（表介绍）；给定 table 返回该表字段清单，每字段含 name（**落库线名**，可直接喂 \
             db_query 的 field）/ type（类型文本）/ doc（字段说明），不吐清单字段。可选 scope 收窄维度\
             （\"user\" = 全局库，\"workspace\" = 当前工作区库），缺省两库全列；带 table 时按表所属维度解析"
            .to_owned(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "scope": { "type": "string", "enum": ["user", "workspace"], "description": "收窄维度（缺省两库全列）" },
                "table": { "type": "string", "description": "表名（省略 = 表清单；给出 = 该表字段说明）" }
            },
            "required": []
        }),
    }
}

/// `db_query` 定义（取数：声明式条件 / 排序 / 投影 / 分页，语义单源在 store）。
fn db_query_definition() -> ToolDefinition {
    ToolDefinition {
        name: DB_QUERY_TOOL.to_owned(),
        description: format!(
            "按条件取本机数据（只读）。scope 指定取哪一库：\"user\" = 全局库（跨工作区共享的表）、\
             \"workspace\" = 当前工作区库（**缺省**）；表不属所选维度时按报错提示补 scope。\
             字段名用**落库线名**（camelCase，如 changeId 而非 change_id；\
             写错会报错并列出候选线名，拿不准先 db_tables 下钻）。where 为条件数组、**多项 AND**（无 OR）；\
             op 词表：{}；点号下钻支持（event.kind、configSnapshot.model），**数组下标不支持**\
             （event.blocks.0.name 报错）；LIKE 只用 %（前缀 / 后缀 / 包含，无 % 即等值，\\% \\_ 为字面）。\
             order 缺省主键自然序（给出即全量排序后切页，null 恒排末位）；select 为投影路径\
             （key 恒在、不占名额）；offset 缺省 0；limit 缺省 20 / 上限 1000（越界报错、不钳位）。\
             **行数是名义上限、40 KB 字节预算是实际生效值**：一次可能只回 20 余行，请据返回体的 \
             total（表全量）/ matched（命中）/ returned（本页）与 truncatedBy 判断，不要据 returned \
             推断库里有多少。注意 session / session_event 是**本会话自身转录**，查询会把当前对话\
             再次喂回上下文，慎查",
            vocabulary(store::QUERY_OPS)
        ),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "table": { "type": "string", "description": "表名（db_tables 清单取值）" },
                "scope": { "type": "string", "enum": ["user", "workspace"], "description": "取哪一库（缺省 workspace）" },
                "where": {
                    "type": "array",
                    "description": "条件数组（AND 组合）",
                    "items": {
                        "type": "object",
                        "properties": {
                            "field": { "type": "string", "description": "落库线名（支持点号下钻，不支持数组下标）" },
                            "op": { "type": "string", "enum": store::QUERY_OPS, "description": "比较符（封闭词表）" },
                            "value": { "description": "比较值（IN 须为数组；IS NULL / IS NOT NULL 不接）" }
                        },
                        "required": ["field", "op"]
                    }
                },
                "order": {
                    "type": "object",
                    "description": "排序（缺省主键自然序）",
                    "properties": {
                        "field": { "type": "string", "description": "排序字段路径" },
                        "desc": { "type": "boolean", "description": "降序（缺省升序；null 恒排末位）" }
                    },
                    "required": ["field"]
                },
                "select": {
                    "type": "array",
                    "description": "投影路径集（key 恒在）",
                    "items": { "type": "string" }
                },
                "offset": { "type": "integer", "description": "深翻偏移（缺省 0）" },
                "limit": { "type": "integer", "description": "本页行数上限（缺省 20、上限 1000；预算会先生效）" }
            },
            "required": ["table"]
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
                          护栏，超时即终止进程树；每次调用必须给 intent（缺失或空白即拒绝执行）"
                .to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "待执行的命令行（git-bash / sh unix 语法优先，探测失败退 cmd）" },
                    "intent": { "type": "string", "description": "一句话说明这条命令要干什么——折叠状态下用户只读这一行（与 Bash 同排展示，超长自动省略）。建议不超过 30 字，写目的而非复述命令文本" },
                    "timeout_ms": { "type": "integer", "description": "超时毫秒（缺省 120000；须在 1000-1800000 内，越界报错不执行，超时终止进程树）" }
                },
                "required": ["command", "intent"]
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
        protect: None,
        before_done: None,
    }
}
