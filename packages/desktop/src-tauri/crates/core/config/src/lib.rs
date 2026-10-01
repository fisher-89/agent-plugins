//! 工作区配置 crate：读取 + 解析 + 校验 + 默认值填充。
//!
//! 配置语义的唯一实现，校验规则逐条复刻 CLI zod schema
//! （`config.schema.ts`：框架八值枚举 / 0–100 值域 / suite root 通配符禁令 /
//! `schema` 字面量 / prefault 默认值 / 顶层 passthrough）；CLI `readConfig`
//! 的「非法即静默整体回默认」吞错语义**不复刻**——任意输入（文件缺失 /
//! 读取失败 / JSON 语法非法 / 字段非法）MUST NOT 使 [`load`] 失败收场，
//! 恒产出「永远合法」的 config + diagnostics 信封：违例字段以默认值填充、
//! 细节进 diagnostics，未来模块永远拿到合法配置。
//!
//! 本 crate 是工作区配置的唯一合法出口（desktop-crate-layout 依赖规则）：
//! 未来任何需要配置的模块仅准经 [`load`] 获取配置项，MUST NOT 自行读取
//! 配置文件。路径一律经 `foundation::layout::config_path` 取得，crate 内
//! 零路径拼接、零文件名字面量（layout_test 命名隔离扫描执法）。
//!
//! 自洽不变量（「config 输出永远合法」的机械形态）：任意输入产出的
//! [`WorkspaceConfig`] 经 `serde_json::to_value` 回写后再过同一 [`assemble`]，
//! diagnostics 必为空。机械前提是 **null 位语义**：DTO 全家为纯 derive
//! serde（与代码库线面惯例一致），`None` 字段线面序列化为 `null`，因此
//! 组装层对**可选（Option）字段位**的显式 `null` 与键缺失同处置（视为未
//! 设、无诊断）；null 在其余位置（阈值 / 容器 / 必需字段 / 顶层）仍为违例。
//!
//! 能力 spec：`specs/desktop-workspace-config/spec.md`（路径相对域根）。

use std::{fs, path::Path};

use foundation::layout::config_path;
use serde::Serialize;
use serde_json::{Map, Value};
use specta::Type;

/// `schema` 字段字面量（CLI zod `literal('spec-driven')` 对齐）
const SCHEMA_LITERAL: &str = "spec-driven";

/// suite 执行目录默认值（CLI `prefault('.')` 对齐）
const SUITE_CWD_DEFAULT: &str = ".";

/// 行覆盖率阈值默认值（CLI `defaults.ts` 对齐）
const COVERAGE_LINES_DEFAULT: f64 = 80.0;

/// 分支覆盖率阈值默认值（CLI `defaults.ts` 对齐）
const COVERAGE_BRANCHES_DEFAULT: f64 = 70.0;

/// 函数覆盖率阈值默认值（CLI `defaults.ts` 对齐）
const COVERAGE_FUNCTIONS_DEFAULT: f64 = 75.0;

/// 变异测试得分阈值默认值（CLI `defaults.ts` 对齐）
const MUTATION_SCORE_DEFAULT: f64 = 70.0;

/// 顶层已知字段集合：其余键全部 passthrough 进 [`WorkspaceConfig::extra`]。
const KNOWN_KEYS: [&str; 7] = [
    "$schema",
    "schema",
    "context",
    "rules",
    "static_analysis",
    "tests",
    "write_protection",
];

/// 测试框架八值枚举（CLI zod enum 同序同串；逐变体显式 rename 对齐 CLI 串）
#[derive(Debug, Clone, Serialize, Type)]
pub enum TestFramework {
    #[serde(rename = "jest")]
    Jest,
    #[serde(rename = "vitest")]
    Vitest,
    #[serde(rename = "vite-plus")]
    VitePlus,
    #[serde(rename = "bun")]
    Bun,
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "node-test")]
    NodeTest,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "pytest")]
    Pytest,
}

/// 覆盖率阈值（百分点 0–100）：三阈值恒有值——文件显式设值原样保留，
/// 未设 / 非法吃默认（80 / 70 / 75）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverageThresholds {
    /// 行覆盖率阈值（默认 80）
    pub lines: f64,
    /// 分支覆盖率阈值（默认 70）
    pub branches: f64,
    /// 函数覆盖率阈值（默认 75）
    pub functions: f64,
}

impl Default for CoverageThresholds {
    /// 全默认基线（CLI `defaults.ts` 对齐）：80 / 70 / 75。
    fn default() -> Self {
        Self {
            lines: COVERAGE_LINES_DEFAULT,
            branches: COVERAGE_BRANCHES_DEFAULT,
            functions: COVERAGE_FUNCTIONS_DEFAULT,
        }
    }
}

/// 变异测试配置：`score` 恒有值（未设 / 非法吃默认 70）；`cwd` 无默认。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MutationConfig {
    /// 突变执行目录（相对 root；未设为 `None`，由消费者按 LCA 计算）
    pub cwd: Option<String>,
    /// 变异测试得分阈值（默认 70）
    pub score: f64,
}

impl Default for MutationConfig {
    /// 全默认基线（CLI `defaults.ts` 对齐）：`cwd` 未设、score 70。
    fn default() -> Self {
        Self {
            cwd: None,
            score: MUTATION_SCORE_DEFAULT,
        }
    }
}

/// 测试 suite 配置（合法 suite 原样保留 + 默认值填充；`root` / `framework`
/// 非法的 suite 整体剔除并记 diagnostics）
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestSuite {
    /// suite 锚点路径（相对项目根，非空且无 glob 通配符）
    pub root: String,
    /// 测试框架
    pub framework: TestFramework,
    /// 执行目录（相对 root，默认 `.`）
    pub cwd: String,
    /// 框架配置文件路径（相对 root；未设为 `None`）
    pub config: Option<String>,
    /// 匹配 glob 列表（相对 root；未设为 `None`）
    pub includes: Option<Vec<String>>,
    /// 排除 glob 列表（相对 root；未设为 `None`）
    pub excludes: Option<Vec<String>>,
    /// 覆盖率阈值（三阈值恒有值）
    pub coverage: CoverageThresholds,
    /// 变异测试配置（score 恒有值）
    pub mutation: MutationConfig,
}

/// 自定义规则配置，按工作流阶段分组；子字段为字符串列表或未设。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RulesConfig {
    /// 提案阶段的自定义规则列表（未设为 `None`）
    pub proposal: Option<Vec<String>>,
    /// 任务阶段的自定义规则列表（未设为 `None`）
    pub tasks: Option<Vec<String>>,
}

/// 写入保护配置。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WriteProtection {
    /// 需要保护的文件 glob 模式列表（未设为 `None`）
    pub files: Option<Vec<WriteProtectionFile>>,
}

/// 单条写入保护规则；`glob` 非空串或非字符串的整条剔除并记 diagnostics。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WriteProtectionFile {
    /// 文件路径 glob 模式（未设为 `None`）
    pub glob: Option<String>,
    /// 自定义拒绝原因（未设为 `None`）
    pub reason: Option<String>,
}

/// 顶层未知字段 passthrough 载体：key + 原文值原样保留、无诊断。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigExtraField {
    /// 未知字段名
    pub key: String,
    /// 原文值（嵌套形态无损保留；出线 TS `unknown`）
    pub value: Value,
}

/// 永远合法的完整工作区配置：合法字段原样保留、违例字段吃默认、未知字段
/// 进 `extra`。只序列化方向（组装层 `assemble` 从原文 `Value` 逐字段判定
/// 处置，本类型从不反序列化）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    /// schema 规则文件引用（线面字段名 `$schema`）
    #[serde(rename = "$schema")]
    pub schema_ref: Option<String>,
    /// 模式标识，固定为 `spec-driven`（未设 / 非法吃默认）
    pub schema: String,
    /// 项目上下文描述（未设为 `None`）
    pub context: Option<String>,
    /// 自定义规则配置（未设为 `None`）
    pub rules: Option<RulesConfig>,
    /// 静态分析工具配置（未设为 `None`）
    pub static_analysis: Option<String>,
    /// 测试 suite 配置列表（保序；违例 suite 剔除）
    pub tests: Vec<TestSuite>,
    /// 写入保护配置（未设为 `None`）
    pub write_protection: Option<WriteProtection>,
    /// 顶层未知字段 passthrough（key + 原文值）
    pub extra: Vec<ConfigExtraField>,
}

impl Default for WorkspaceConfig {
    /// 全默认基线（CLI `defaults.ts` 对齐）：`schema` 为 `spec-driven`、
    /// `tests` 为空列表、其余可选字段未设；供文件级四分支与组装层复用。
    fn default() -> Self {
        Self {
            schema_ref: None,
            schema: SCHEMA_LITERAL.to_owned(),
            context: None,
            rules: None,
            static_analysis: None,
            tests: Vec::new(),
            write_protection: None,
            extra: Vec::new(),
        }
    }
}

/// 诊断类别（前端状态面映射口径）：`fileMissing` → 空态；
/// `readFailed` / `jsonInvalid` → inline 错误；`invalidValue` /
/// `defaultApplied` → 警示区条目。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticKind {
    /// 配置文件不存在（多数 workspace 常态，空态来源、非错误）
    FileMissing,
    /// 配置文件存在但不可读（inline 错误来源）
    ReadFailed,
    /// JSON 语法非法（inline 错误来源）
    JsonInvalid,
    /// 字段违例（已按规则处置）
    InvalidValue,
    /// 字段未设、吃默认值
    DefaultApplied,
}

/// 单条诊断：`path` 为点路径（文件级 `$`，suite 内形如
/// `tests[0].coverage.lines`），`message` 为中文人读文案（含违例原值与
/// 所落默认值）。
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDiagnostic {
    /// 诊断类别
    pub kind: DiagnosticKind,
    /// 违例 / 吃默认字段的点路径
    pub path: String,
    /// 中文人读文案
    pub message: String,
}

/// 配置报告信封（core 域类型，命令线面另有平移 DTO）：`config` 永远合法，
/// `diagnostics` 与 config 同一次组装产出、`path` 可对位 config 字段。
#[derive(Debug, Clone)]
pub struct ConfigReport {
    /// 永远合法的完整配置
    pub config: WorkspaceConfig,
    /// 逐条诊断（文件态 / 违例 / 吃默认）
    pub diagnostics: Vec<ConfigDiagnostic>,
}

/// 原文值人读片段（进 message 的违例原值）：字符串带引号、复合类型只报类型名。
fn describe_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => format!("{b}"),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{s}\""),
        Value::Array(_) => "数组".to_owned(),
        Value::Object(_) => "对象".to_owned(),
    }
}

fn invalid(path: &str, message: String) -> ConfigDiagnostic {
    ConfigDiagnostic {
        kind: DiagnosticKind::InvalidValue,
        path: path.to_owned(),
        message,
    }
}

fn defaulted(path: &str, message: String) -> ConfigDiagnostic {
    ConfigDiagnostic {
        kind: DiagnosticKind::DefaultApplied,
        path: path.to_owned(),
        message,
    }
}

/// 可选字符串字段通用组装：缺失 / 显式 null → `None` 无诊断；字符串 →
/// `Some`；其余非字符串 → `None` + `InvalidValue`。
fn opt_string(
    map: &Map<String, Value>,
    key: &str,
    path: &str,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<String> {
    match map.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(other) => {
            diagnostics.push(invalid(
                path,
                format!(
                    "字段 {key} 必须为字符串（原值 {}），已按未设处理。",
                    describe_value(other)
                ),
            ));
            None
        }
    }
}

/// 可选字符串列表字段通用组装：缺失 / 显式 null → `None` 无诊断；全字符串
/// 数组 → `Some`；非数组或元素混入非字符串 → `None` + `InvalidValue`。
fn opt_string_list(
    map: &Map<String, Value>,
    key: &str,
    path: &str,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<Vec<String>> {
    match map.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::Array(items)) => {
            let mut list = Vec::with_capacity(items.len());
            let mut valid = true;
            for item in items {
                match item.as_str() {
                    Some(entry) => list.push(entry.to_owned()),
                    None => {
                        valid = false;
                        break;
                    }
                }
            }
            if valid {
                Some(list)
            } else {
                diagnostics.push(invalid(
                    path,
                    format!("字段 {key} 必须为字符串数组（存在非字符串元素），已按未设处理。"),
                ));
                None
            }
        }
        Some(other) => {
            diagnostics.push(invalid(
                path,
                format!(
                    "字段 {key} 必须为字符串数组（原值 {}），已按未设处理。",
                    describe_value(other)
                ),
            ));
            None
        }
    }
}

/// 数值阈值字段通用组装（zod `number().min(0).max(100).optional().prefault(N)`
/// 移植）：缺失 → 默认 + `DefaultApplied`；0–100 内数值原样；越界或非数值
/// → 默认 + `InvalidValue`。
fn threshold(
    map: &Map<String, Value>,
    key: &str,
    default: f64,
    path: &str,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> f64 {
    match map.get(key) {
        None => {
            diagnostics.push(defaulted(
                path,
                format!("阈值 {key} 未设置，使用默认值 {default}。"),
            ));
            default
        }
        Some(value) => match value.as_f64() {
            Some(number) if (0.0..=100.0).contains(&number) => number,
            _ => {
                diagnostics.push(invalid(
                    path,
                    format!(
                        "阈值 {key} 必须为 0–100 的数值（原值 {}），使用默认值 {default}。",
                        describe_value(value)
                    ),
                ));
                default
            }
        },
    }
}

/// CLI zod 枚举串 → 框架枚举（八值逐一对位）。
fn parse_framework(value: &str) -> Option<TestFramework> {
    match value {
        "jest" => Some(TestFramework::Jest),
        "vitest" => Some(TestFramework::Vitest),
        "vite-plus" => Some(TestFramework::VitePlus),
        "bun" => Some(TestFramework::Bun),
        "rust" => Some(TestFramework::Rust),
        "node-test" => Some(TestFramework::NodeTest),
        "go" => Some(TestFramework::Go),
        "pytest" => Some(TestFramework::Pytest),
        _ => None,
    }
}

/// suite 元素组装：`root` / `framework` 必需字段缺失或非法 → 整 suite 剔除
/// （`None`）+ `InvalidValue`；其余字段按 prefault / 类型规则逐字段处置。
fn assemble_suite(
    item: &Value,
    index: usize,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<TestSuite> {
    let suite_path = format!("tests[{index}]");
    let Some(map) = item.as_object() else {
        diagnostics.push(invalid(
            &suite_path,
            format!(
                "suite 必须为对象（原值 {}），该 suite 已被剔除。",
                describe_value(item)
            ),
        ));
        return None;
    };

    // 必需字段 root：非空且无 glob 通配符（`*` `?` `{` `[`），违例剔除整 suite
    let root_path = format!("{suite_path}.root");
    let root = match map.get("root") {
        None => {
            diagnostics.push(invalid(
                &root_path,
                "suite 必需字段 root 缺失，该 suite 已被剔除。".to_owned(),
            ));
            return None;
        }
        Some(Value::String(root)) if !root.is_empty() && !root.contains(['*', '?', '{', '[']) => {
            root.clone()
        }
        Some(other) => {
            diagnostics.push(invalid(
                &root_path,
                format!(
                    "suite root 必须为非空字符串且不含 glob 通配符（原值 {}），该 suite 已被剔除。",
                    describe_value(other)
                ),
            ));
            return None;
        }
    };

    // 必需字段 framework：八值枚举，违例剔除整 suite
    let framework_path = format!("{suite_path}.framework");
    let framework = match map.get("framework") {
        None => {
            diagnostics.push(invalid(
                &framework_path,
                "suite 必需字段 framework 缺失，该 suite 已被剔除。".to_owned(),
            ));
            return None;
        }
        Some(value @ Value::String(_)) => match value.as_str().and_then(parse_framework) {
            Some(framework) => framework,
            None => {
                diagnostics.push(invalid(
                    &framework_path,
                    format!(
                        "suite framework 必须为受支持的框架枚举值（原值 {}），该 suite 已被剔除。",
                        describe_value(value)
                    ),
                ));
                return None;
            }
        },
        Some(other) => {
            diagnostics.push(invalid(
                &framework_path,
                format!(
                    "suite framework 必须为受支持的框架枚举值（原值 {}），该 suite 已被剔除。",
                    describe_value(other)
                ),
            ));
            return None;
        }
    };

    // cwd：prefault('.')——缺失吃默认 + DefaultApplied，非法吃默认 + InvalidValue
    let cwd_path = format!("{suite_path}.cwd");
    let cwd = match map.get("cwd") {
        None => {
            diagnostics.push(defaulted(
                &cwd_path,
                format!("suite cwd 未设置，使用默认值 \"{SUITE_CWD_DEFAULT}\"。"),
            ));
            SUITE_CWD_DEFAULT.to_owned()
        }
        Some(Value::String(cwd)) => cwd.clone(),
        Some(other) => {
            diagnostics.push(invalid(
                &cwd_path,
                format!(
                    "suite cwd 必须为字符串（原值 {}），使用默认值 \"{SUITE_CWD_DEFAULT}\"。",
                    describe_value(other)
                ),
            ));
            SUITE_CWD_DEFAULT.to_owned()
        }
    };

    // config：nonempty 可选——空串或非字符串 → None + InvalidValue，缺失 / null 无诊断
    let config_path = format!("{suite_path}.config");
    let config = match map.get("config") {
        None | Some(Value::Null) => None,
        Some(Value::String(config)) if !config.is_empty() => Some(config.clone()),
        Some(other) => {
            diagnostics.push(invalid(
                &config_path,
                format!(
                    "suite config 必须为非空字符串（原值 {}），已按未设处理。",
                    describe_value(other)
                ),
            ));
            None
        }
    };

    let includes_path = format!("{suite_path}.includes");
    let includes = opt_string_list(map, "includes", &includes_path, diagnostics);
    let excludes_path = format!("{suite_path}.excludes");
    let excludes = opt_string_list(map, "excludes", &excludes_path, diagnostics);

    // coverage：对象级 prefault({})——缺失三阈值逐字段 DefaultApplied；
    // 非对象三阈值全默认 + InvalidValue；对象内逐阈值走 threshold 组装
    let coverage_path = format!("{suite_path}.coverage");
    let coverage = match map.get("coverage") {
        None => {
            for (key, default) in [
                ("lines", COVERAGE_LINES_DEFAULT),
                ("branches", COVERAGE_BRANCHES_DEFAULT),
                ("functions", COVERAGE_FUNCTIONS_DEFAULT),
            ] {
                diagnostics.push(defaulted(
                    &format!("{coverage_path}.{key}"),
                    format!("覆盖率阈值 {key} 未设置，使用默认值 {default}。"),
                ));
            }
            CoverageThresholds::default()
        }
        Some(Value::Object(coverage_map)) => CoverageThresholds {
            lines: threshold(
                coverage_map,
                "lines",
                COVERAGE_LINES_DEFAULT,
                &format!("{coverage_path}.lines"),
                diagnostics,
            ),
            branches: threshold(
                coverage_map,
                "branches",
                COVERAGE_BRANCHES_DEFAULT,
                &format!("{coverage_path}.branches"),
                diagnostics,
            ),
            functions: threshold(
                coverage_map,
                "functions",
                COVERAGE_FUNCTIONS_DEFAULT,
                &format!("{coverage_path}.functions"),
                diagnostics,
            ),
        },
        Some(other) => {
            diagnostics.push(invalid(
                &coverage_path,
                format!(
                    "coverage 必须为对象（原值 {}），三个覆盖率阈值均使用默认值。",
                    describe_value(other)
                ),
            ));
            CoverageThresholds::default()
        }
    };

    // mutation：对象级 prefault({})——缺失 cwd None（无诊断）+ score 默认
    // DefaultApplied；非对象同缺失处置 + InvalidValue；对象内逐字段组装
    let mutation_path = format!("{suite_path}.mutation");
    let score_defaulted = |diagnostics: &mut Vec<ConfigDiagnostic>| {
        diagnostics.push(defaulted(
            &format!("{mutation_path}.score"),
            format!("变异测试得分阈值 score 未设置，使用默认值 {MUTATION_SCORE_DEFAULT}。"),
        ));
    };
    let mutation = match map.get("mutation") {
        None => {
            score_defaulted(diagnostics);
            MutationConfig::default()
        }
        Some(Value::Object(mutation_map)) => MutationConfig {
            cwd: opt_string(
                mutation_map,
                "cwd",
                &format!("{mutation_path}.cwd"),
                diagnostics,
            ),
            score: threshold(
                mutation_map,
                "score",
                MUTATION_SCORE_DEFAULT,
                &format!("{mutation_path}.score"),
                diagnostics,
            ),
        },
        Some(other) => {
            diagnostics.push(invalid(
                &mutation_path,
                format!(
                    "mutation 必须为对象（原值 {}），得分阈值使用默认值。",
                    describe_value(other)
                ),
            ));
            score_defaulted(diagnostics);
            MutationConfig::default()
        }
    };

    Some(TestSuite {
        root,
        framework,
        cwd,
        config,
        includes,
        excludes,
        coverage,
        mutation,
    })
}

/// 写入保护规则组装：元素非对象或 `glob` 非空串违例 → 该条剔除 + `InvalidValue`；
/// `reason` 非字符串 → `None` + `InvalidValue`；合法条目原样保留。
fn assemble_file_rule(
    item: &Value,
    index: usize,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<WriteProtectionFile> {
    let rule_path = format!("write_protection.files[{index}]");
    let Some(map) = item.as_object() else {
        diagnostics.push(invalid(
            &rule_path,
            format!(
                "写入保护规则必须为对象（原值 {}），该条规则已被剔除。",
                describe_value(item)
            ),
        ));
        return None;
    };

    let glob = match map.get("glob") {
        None | Some(Value::Null) => None,
        Some(Value::String(glob)) if !glob.is_empty() => Some(glob.clone()),
        Some(other) => {
            diagnostics.push(invalid(
                &format!("{rule_path}.glob"),
                format!(
                    "写入保护规则 glob 必须为非空字符串（原值 {}），该条规则已被剔除。",
                    describe_value(other)
                ),
            ));
            return None;
        }
    };
    let reason = opt_string(map, "reason", &format!("{rule_path}.reason"), diagnostics);
    Some(WriteProtectionFile { glob, reason })
}

/// 组装校验纯函数（design「配置语义移植对照」表逐行落地）：对顶层对象逐字段
/// 判定处置——合法字段原样保留、违例字段按行处置（有默认者吃默认 +
/// `InvalidValue`；suite 必需字段非法剔除整个 suite）、缺失 prefault 字段逐
/// 字段 `DefaultApplied`、顶层未知键收进 `extra`（key + 原文值）。
///
/// 自洽不变量：产出 config 经 `serde_json::to_value` 回写后再过同一
/// [`assemble`]，diagnostics 必为空——`None` 字段线面序列化为 `null`，组装
/// 层对可选字段位的 null 与键缺失同处置（见模块 doc「null 位语义」）。
fn assemble(map: &Map<String, Value>) -> (WorkspaceConfig, Vec<ConfigDiagnostic>) {
    let mut diagnostics: Vec<ConfigDiagnostic> = Vec::new();

    let schema_ref = opt_string(map, "$schema", "$schema", &mut diagnostics);

    // schema 字面量端口：非 `"spec-driven"`（含缺失）一律落默认
    let schema = match map.get("schema") {
        None => {
            diagnostics.push(defaulted(
                "schema",
                format!("schema 字段未设置，使用默认值 \"{SCHEMA_LITERAL}\"。"),
            ));
            SCHEMA_LITERAL.to_owned()
        }
        Some(Value::String(schema)) if schema == SCHEMA_LITERAL => schema.clone(),
        Some(other) => {
            diagnostics.push(invalid(
                "schema",
                format!(
                    "schema 字段必须为字面量 \"{SCHEMA_LITERAL}\"（原值 {}），使用默认值。",
                    describe_value(other)
                ),
            ));
            SCHEMA_LITERAL.to_owned()
        }
    };

    let context = opt_string(map, "context", "context", &mut diagnostics);
    let static_analysis = opt_string(map, "static_analysis", "static_analysis", &mut diagnostics);

    // rules：非对象 → 整体 None + InvalidValue（显式 null 同缺失，无诊断）；
    // 对象内子字段非字符串数组 → 该子字段 None + InvalidValue（path 形如 rules.proposal）
    let rules = match map.get("rules") {
        None | Some(Value::Null) => None,
        Some(Value::Object(rules_map)) => Some(RulesConfig {
            proposal: opt_string_list(rules_map, "proposal", "rules.proposal", &mut diagnostics),
            tasks: opt_string_list(rules_map, "tasks", "rules.tasks", &mut diagnostics),
        }),
        Some(other) => {
            diagnostics.push(invalid(
                "rules",
                format!(
                    "rules 字段必须为对象（原值 {}），已按未设处理。",
                    describe_value(other)
                ),
            ));
            None
        }
    };

    // tests：prefault([])——缺失吃默认 + DefaultApplied；非数组吃默认 +
    // InvalidValue；元素逐个组装，违例 suite 剔除、合法 suite 保序保留
    let tests = match map.get("tests") {
        None => {
            diagnostics.push(defaulted(
                "tests",
                "tests 字段未设置，使用空 suite 列表 []。".to_owned(),
            ));
            Vec::new()
        }
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| assemble_suite(item, index, &mut diagnostics))
            .collect(),
        Some(other) => {
            diagnostics.push(invalid(
                "tests",
                format!(
                    "tests 字段必须为数组（原值 {}），使用空 suite 列表 []。",
                    describe_value(other)
                ),
            ));
            Vec::new()
        }
    };

    // write_protection：非对象 → None + InvalidValue（显式 null 同缺失）；对象内
    // files 非数组 → files None + InvalidValue（path write_protection.files）；逐条规则组装
    let write_protection = match map.get("write_protection") {
        None | Some(Value::Null) => None,
        Some(Value::Object(protection_map)) => match protection_map.get("files") {
            None | Some(Value::Null) => Some(WriteProtection { files: None }),
            Some(Value::Array(items)) => Some(WriteProtection {
                files: Some(
                    items
                        .iter()
                        .enumerate()
                        .filter_map(|(index, item)| {
                            assemble_file_rule(item, index, &mut diagnostics)
                        })
                        .collect(),
                ),
            }),
            Some(other) => {
                diagnostics.push(invalid(
                    "write_protection.files",
                    format!(
                        "write_protection.files 必须为数组（原值 {}），已按未设处理。",
                        describe_value(other)
                    ),
                ));
                Some(WriteProtection { files: None })
            }
        },
        Some(other) => {
            diagnostics.push(invalid(
                "write_protection",
                format!(
                    "write_protection 字段必须为对象（原值 {}），已按未设处理。",
                    describe_value(other)
                ),
            ));
            None
        }
    };

    // 顶层未知字段 passthrough：key + 原文值原样保留，无诊断
    let extra = map
        .iter()
        .filter(|(key, _)| !KNOWN_KEYS.contains(&key.as_str()))
        .map(|(key, value)| ConfigExtraField {
            key: key.clone(),
            value: value.clone(),
        })
        .collect();

    (
        WorkspaceConfig {
            schema_ref,
            schema,
            context,
            rules,
            static_analysis,
            tests,
            write_protection,
            extra,
        },
        diagnostics,
    )
}

/// 配置语义唯一入口：经 `foundation::layout::config_path` 取配置文件路径
/// → 文件态四分支全走默认值报告（缺失 `FileMissing` / 读取失败 `ReadFailed`
/// / 语法非法 `JsonInvalid` / 顶层非对象 `InvalidValue`）→ [`assemble`]
/// 逐字段组装校验。任意输入不失败收场；不校验 root 有效性（命令层职责）。
pub fn load(root: &Path) -> ConfigReport {
    let path = config_path(root);
    let (config, diagnostics) = match fs::read_to_string(&path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (
            WorkspaceConfig::default(),
            vec![ConfigDiagnostic {
                kind: DiagnosticKind::FileMissing,
                path: "$".to_owned(),
                message: "工作区配置文件不存在，呈现全部默认值。".to_owned(),
            }],
        ),
        Err(err) => (
            WorkspaceConfig::default(),
            vec![ConfigDiagnostic {
                kind: DiagnosticKind::ReadFailed,
                path: "$".to_owned(),
                message: format!("工作区配置文件读取失败（{err}），呈现全部默认值。"),
            }],
        ),
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Err(err) => (
                WorkspaceConfig::default(),
                vec![ConfigDiagnostic {
                    kind: DiagnosticKind::JsonInvalid,
                    path: "$".to_owned(),
                    message: format!("工作区配置文件不是合法 JSON（{err}），呈现全部默认值。"),
                }],
            ),
            Ok(Value::Object(map)) => assemble(&map),
            Ok(other) => (
                WorkspaceConfig::default(),
                vec![invalid(
                    "$",
                    format!(
                        "工作区配置顶层必须为对象（原值 {}），呈现全部默认值。",
                        describe_value(&other)
                    ),
                )],
            ),
        },
    };
    ConfigReport {
        config,
        diagnostics,
    }
}

#[cfg(test)]
mod lib_test;
