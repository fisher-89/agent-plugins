//! 模型目录：provider 管理页的**建议面**（「这家协议有哪些常用模型 / 它的标称
//! 窗长与最大输出是多少 / 它支持哪些 effort 值」）。
//!
//! 目录**不参与保存期校验、不参与运行期解析**：它只是前端参考数据，漂移代价
//! 上限是「建议不准」，不会变成「同一窗长两套算法」——运行期取值链仍是
//! `spec.length` → `context_length` → 128K 与 `max_tokens` → rig 名单 → 窗长
//! 分档，保存期只校验模型名后缀与输出上限下限，从不查目录成员。
//!
//! 零凭据 / 零端点（纯静态参考数据，`Debug` 可派生、无遮蔽需求）；名单手写于
//! 编译期，过时是**知情代价**：形状（两级目录 + 五条不变量）才是契约，名单不是。

use serde::Serialize;

use store::AgentProviderProtocol;

/// 两级目录根（`protocols` 非空、每条协议 `models` 非空）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalog {
    /// 按协议分组的条目段（每段含协议级 effort 建议 + 模型条目）
    pub protocols: Vec<CatalogProtocol>,
}

/// 单协议段：协议级 effort **建议**值域 + 该协议的常用模型条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProtocol {
    /// 协议（复用 store 线值枚举——「协议」不造第二份真源）
    pub protocol: AgentProviderProtocol,
    /// 协议级 effort **建议**值域（非校验值域：保存期不查成员，自定义值必须能存）
    pub efforts: Vec<String>,
    /// 该协议的常用模型条目（非空）
    pub models: Vec<CatalogModel>,
}

/// 单模型条目：完整模型原文 + 能力元数据（预填用）+ 模型级 effort 覆盖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    /// 完整模型原文（可含 `[1m]` 后缀，MUST NOT 另造「后缀位」或第二份裸名）
    pub value: String,
    /// 标称上下文窗长（token，建议预填值；缺席 = `null` = 该输入框不预填）。
    /// `value` 含 `[1m]` 后缀的条目 MUST 为 `1_000_000`（目录内不自相矛盾）
    pub context_length: Option<u64>,
    /// 标称最大输出（token，建议预填值；缺席 = `null` = 不预填）。`openai`
    /// 协议的请求恒不写上限，前端也不预填该值（取值链不受影响）
    pub max_tokens: Option<u64>,
    /// 是否支持思考强度（只影响建议面：`false` ⇒ 该条目 effort 建议为空）
    pub supports_thinking: bool,
    /// 模型级 effort 覆盖：`Some(list)`（含 `Some([])` = 显式「无建议」）优先于
    /// 协议级，`None` = 继承协议级
    pub efforts: Option<Vec<String>>,
}

/// 目录读数：纯静态表、零 IO、零参数、零凭据。每次返回**新构造**的对象——
/// 体量是常量级结构，构造成本低于一次 IPC 序列化（不引共享缓存单例：那是一层
/// 共享可变边界与生命周期负担，换不到这一层收益）。
pub fn catalog() -> ModelCatalog {
    ModelCatalog {
        protocols: vec![
            CatalogProtocol {
                protocol: AgentProviderProtocol::Openai,
                // DeepSeek 的深度档是 `low / high / max`（`medium` 在其接口里
                // 映射成 high，是假三档）；GPT-6 系不在此列——rig 的
                // `refuse_tools_while_reasoning` 使该族带 tools 时只接受「关思考」
                //（`none`），而 `none` 不在本目录值域内
                efforts: vec!["low".to_owned(), "high".to_owned(), "max".to_owned()],
                models: vec![
                    CatalogModel {
                        value: "deepseek-chat".to_owned(),
                        context_length: Some(128 * 1024),
                        max_tokens: Some(8_192),
                        supports_thinking: false,
                        efforts: Some(Vec::new()),
                    },
                    CatalogModel {
                        value: "deepseek-chat[1m]".to_owned(),
                        context_length: Some(1_000_000),
                        max_tokens: Some(8_192),
                        supports_thinking: false,
                        efforts: Some(Vec::new()),
                    },
                    CatalogModel {
                        value: "deepseek-reasoner".to_owned(),
                        context_length: Some(128 * 1024),
                        max_tokens: Some(64 * 1024),
                        supports_thinking: true,
                        efforts: None,
                    },
                    CatalogModel {
                        value: "deepseek-reasoner[1m]".to_owned(),
                        context_length: Some(1_000_000),
                        max_tokens: Some(64 * 1024),
                        supports_thinking: true,
                        efforts: None,
                    },
                ],
            },
            CatalogProtocol {
                protocol: AgentProviderProtocol::Anthropic,
                // 官方 effort 值域（含 `medium` / `xhigh`）：比 openai 侧宽，
                // 正是「efforts 不做封闭值域校验」的事实依据
                efforts: vec![
                    "low".to_owned(),
                    "medium".to_owned(),
                    "high".to_owned(),
                    "xhigh".to_owned(),
                    "max".to_owned(),
                ],
                models: vec![
                    CatalogModel {
                        value: "claude-opus-4-6".to_owned(),
                        context_length: Some(200_000),
                        max_tokens: Some(64 * 1024),
                        supports_thinking: true,
                        efforts: None,
                    },
                    CatalogModel {
                        value: "claude-opus-4-6[1m]".to_owned(),
                        context_length: Some(1_000_000),
                        max_tokens: Some(64 * 1024),
                        supports_thinking: true,
                        efforts: None,
                    },
                    CatalogModel {
                        value: "claude-sonnet-4-6".to_owned(),
                        context_length: Some(200_000),
                        max_tokens: Some(64 * 1024),
                        supports_thinking: true,
                        efforts: None,
                    },
                    CatalogModel {
                        value: "claude-sonnet-4-6[1m]".to_owned(),
                        context_length: Some(1_000_000),
                        max_tokens: Some(64 * 1024),
                        supports_thinking: true,
                        efforts: None,
                    },
                    CatalogModel {
                        value: "claude-3-5-haiku-latest".to_owned(),
                        context_length: Some(200_000),
                        max_tokens: Some(8_192),
                        supports_thinking: false,
                        efforts: Some(Vec::new()),
                    },
                ],
            },
        ],
    }
}
