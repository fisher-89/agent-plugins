use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::bindings::export_bindings;

/// 产物文件路径（与 `export_bindings` 内 `CARGO_MANIFEST_DIR` 定位同式）。
fn generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 应有父目录")
        .join("src/types/generated/bindings.ts")
}

/// 产物文件互斥锁（cargo test 多线程并行，单一共享产物必须串行访问）。
fn file_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    file_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 权威产物快照：重导出（幂等确定性）后读取字节。
fn authoritative_snapshot() -> Vec<u8> {
    export_bindings().expect("export_bindings 应成功");
    fs::read(generated_path()).expect("读取产物文件应成功")
}

/// 变更类用例的 Drop 守卫：断言失败 unwind 时尽力重导出恢复权威产物
/// （仅在持有 [`file_lock`] 的用例内使用，不重复加锁）。
struct RestoreOnDrop;

impl Drop for RestoreOnDrop {
    fn drop(&mut self) {
        let _ = export_bindings();
    }
}

/// 32 条命令的生成包装名（camelCase，与 `generate_handler!` 时代命令清单一一
/// 对应；历史缺录为既有滞后债，contains 语义不致红、按现行口径只补录本变更
/// 自身新命令）。
const COMMAND_WRAPPERS: &[&str] = &[
    "listChanges",
    "getChangeDetail",
    "readArtifact",
    "archiveChange",
    "listWorkspaces",
    "addWorkspace",
    "removeWorkspace",
    "listAgentProviders",
    "saveAgentProvider",
    "deleteAgentProvider",
    "listAgentInstances",
    "saveAgentInstance",
    "deleteAgentInstance",
    "setDefaultAgentInstance",
    "agentStart",
    "agentStop",
    "agentSessions",
    "agentSessionTranscript",
    "sessionDetail",
    "readExplore",
    "scanExplores",
    "exploreDocPath",
    "listExploreRecords",
    "createExploreRecord",
    "renameExploreRecord",
    "deleteExploreRecord",
    "watchSubscribe",
    "watchUnsubscribe",
    "dbModels",
    "dbRecords",
    "codeStats",
    "workspaceConfig",
];

/// 32 条命令的 IPC 命令名（snake_case，invoke 目标；补录口径同上）。
const COMMAND_NAMES: &[&str] = &[
    "list_changes",
    "get_change_detail",
    "read_artifact",
    "archive_change",
    "list_workspaces",
    "add_workspace",
    "remove_workspace",
    "list_agent_providers",
    "save_agent_provider",
    "delete_agent_provider",
    "list_agent_instances",
    "save_agent_instance",
    "delete_agent_instance",
    "set_default_agent_instance",
    "agent_start",
    "agent_stop",
    "agent_sessions",
    "agent_session_transcript",
    "session_detail",
    "read_explore",
    "scan_explores",
    "explore_doc_path",
    "list_explore_records",
    "create_explore_record",
    "rename_explore_record",
    "delete_explore_record",
    "watch_subscribe",
    "watch_unsubscribe",
    "db_models",
    "db_records",
    "code_stats",
    "workspace_config",
];

/// 全部出线 DTO 类型名（产物 Types 段的 `export type` 全集）。
const DTO_TYPES: &[&str] = &[
    "ActivePhase",
    "AgentBlock",
    "AgentEngineKind",
    "AgentEvent",
    "AgentEventKind",
    "AgentInstanceRecord",
    "AgentModelTiers",
    "AgentPermissionMode",
    "AgentProviderRecord",
    "AgentRunMessage",
    "AgentRunStatus",
    "SessionRow",
    "SessionStats",
    "SessionSummary",
    "TurnSummary",
    "ArchiveGroup",
    "ArchiveOutcome",
    "CreateOutcome",
    "ArtifactDescriptor",
    "ArtifactEnvelope",
    "AttemptRecord",
    "ChangeDetail",
    "ChangeList",
    "ChangeSource",
    "ChangeStatus",
    "ChangeSummary",
    "ChecklistItem",
    "CodeStatsReport",
    "CodeTotals",
    "ConfigDiagnostic",
    "ConfigExtraField",
    "CoverageThresholds",
    "DiagnosticKind",
    "DirNode",
    "ExploreDoc",
    "ExploreRecord",
    "ExploreScanEntry",
    "FileNode",
    "FileWatchEvent",
    "LanguageStats",
    "ModelInfo",
    "MutationConfig",
    "PhaseEntry",
    "RecordEnvelope",
    "RulesConfig",
    "TestFramework",
    "TestSuite",
    "TreeEntry",
    "Verdict",
    "WorkspaceConfig",
    "WorkspaceConfigReport",
    "WorkspaceRecord",
    "WriteProtection",
    "WriteProtectionFile",
];

/// 产物中某 `export type` 声明的完整文本段（自声明起至下一个顶层 `export` 前），
/// 供逐字段出线形态对位（避免全文 contains 误命中同名字段）。
fn type_section(content: &str, type_name: &str) -> String {
    let marker = format!("export type {type_name} =");
    let start = content
        .find(&marker)
        .unwrap_or_else(|| panic!("产物缺出线类型 {type_name}"));
    let body_start = start + marker.len();
    let end = content[body_start..]
        .find("\nexport ")
        .map_or(content.len(), |offset| body_start + offset);
    content[start..end].to_owned()
}

// ---------------------------------------------------------------------------
// export_bindings 覆盖性（AC-3）
// ---------------------------------------------------------------------------

#[test]
fn 导出产物包含全部32条命令包装名与invoke命令名及出线dto类型名() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 全部命令经同一 `commands` 对象出线
    assert!(
        content.contains("export const commands = {"),
        "产物含 commands 注册面对象"
    );
    for wrapper in COMMAND_WRAPPERS {
        assert!(
            content.contains(&format!("{wrapper}:")),
            "产物缺命令包装名 {wrapper}"
        );
    }
    for name in COMMAND_NAMES {
        assert!(
            content.contains(&format!("\"{name}\"")),
            "产物缺 invoke 命令名 {name}"
        );
    }
    for dto in DTO_TYPES {
        assert!(
            content.contains(&format!("export type {dto} =")),
            "产物缺出线 DTO 类型 {dto}"
        );
    }
}

#[test]
fn agent_start与watch_subscribe绑定为typed_channel参数() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // agent_start：Channel<AgentRunMessage> typed 参数 + 信封类型来自生成物
    assert!(
        content.contains("agentStart: (onEvent: Channel<AgentRunMessage>,"),
        "agentStart 绑定首参为 typed Channel<AgentRunMessage>"
    );
    assert!(
        content.contains("__TAURI_INVOKE<TurnSummary>(\"agent_start\""),
        "agent_start invoke 目标与返回类型 typed（提前 resolve 契约返回 running 态轮行）"
    );
    // 信封 ipc 双变体出自生成物（tag `ipc` camelCase；Record 臂载 TurnSummary）
    assert!(content.contains("export type AgentRunMessage ="));
    assert!(content.contains("{ ipc: \"event\"; event: AgentEvent }"));
    assert!(content.contains("{ ipc: \"record\"; record: TurnSummary }"));

    // watch_subscribe：Channel<FileWatchEvent> typed 参数 + 载荷类型来自生成物
    assert!(
        content.contains("watchSubscribe: (onEvent: Channel<FileWatchEvent>, path: string)"),
        "watchSubscribe 绑定为 typed Channel<FileWatchEvent> 参数"
    );
    assert!(
        content.contains("__TAURI_INVOKE<number>(\"watch_subscribe\", { onEvent, path })"),
        "watch_subscribe invoke 透传 onEvent 与 path"
    );
    assert!(content.contains("export type FileWatchEvent ="));
    assert!(content.contains("path: string,"));
}

// ---------------------------------------------------------------------------
// agent 尾参出线形态（AC-3 bindings 再生成含 agent 参数的镜像半边）
// ---------------------------------------------------------------------------

#[test]
fn agent_start绑定签名含尾部agent入参且invoke参数对象恒含agent键() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 尾部 agent 入参（number | null，末位位置参数形态保持；会话域化后
    // sessionId 取代 resumeSessionId/parentRunId）：
    // 「agentStart typed Channel 首参」逐字断言保持，agent 为末位位置参数
    let wrapper_head = "agentStart: (onEvent: Channel<AgentRunMessage>, root: string, prompt: string, permissionMode: AgentPermissionMode, sessionId: string | null, source: string | null, sourceRef: string | null, agent: number | null) => ";
    assert!(
        content.contains(wrapper_head),
        "agentStart 绑定签名以尾部 agent 入参收尾（typed Channel 首参保持）"
    );
    assert!(
        content.contains(
            "\"agent_start\", { onEvent, root, prompt, permissionMode, sessionId, source, sourceRef, agent }"
        ),
        "invoke 参数对象恒含 agent 键与 sessionId 键（会话域化参数面）"
    );
}

#[test]
fn agent_engine_kind出线为cli与sdk字面量联合() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // AgentEngineKind 出线（AC-3 类型镜像半边）：`export type AgentEngineKind =`
    // + 两字面量（serde/specta camelCase 线格式逐字）
    let engine = type_section(&content, "AgentEngineKind");
    assert!(engine.contains("\"cli\""), "实际: {engine}");
    assert!(engine.contains("\"sdk\""), "实际: {engine}");
    assert_eq!(
        engine.matches('|').count(),
        1,
        "二值恰一分隔（不多不少，枚举 +1 侧不外溢）"
    );
    // core 契约零污染：引擎 kind 住 infra 门面 / store 本地枚举，出线不携带
    // engine/rig 字样的 core AgentRunParams 类型（core DTO 清单无
    // EngineRunParams 类条目）
    assert!(
        !content.contains("AgentRunParams"),
        "core AgentRunParams 不出线（core 契约零污染）"
    );
}

#[test]
fn agent_event出线为kind判别联合且seq摊平变体字面量camel_case() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // seq / timestampMs 摊平进信封本体（tag `kind` 扁平：intersection 形态）
    assert!(
        content.contains("export type AgentEvent = {"),
        "AgentEvent 出线为对象形态"
    );
    assert!(content.contains("seq: number,"), "seq 摊平");
    assert!(content.contains("timestampMs: number,"), "timestampMs 摊平");
    assert!(
        content.contains("} & AgentEventKind;"),
        "kind 判别联合经 intersection 摊平（PoC 判据：tag=\"kind\"）"
    );

    // 六变体字面量 camelCase（rename_all_fields 出线形态留档；RunResult →
    // TurnDone 更名 + MessageDelta 增量词汇入列）
    for literal in [
        "kind: \"runStarted\"",
        "kind: \"messageDelta\"",
        "kind: \"message\"",
        "kind: \"systemNotice\"",
        "kind: \"turnDone\"",
        "kind: \"raw\"",
    ] {
        assert!(
            content.contains(literal),
            "AgentEventKind 变体字面量缺失: {literal}"
        );
    }
    // turnDone 变体字段 camelCase（与 dto.ts 现镜像同构）
    for field in [
        "isError: boolean;",
        "numTurns: number | null;",
        "costUsd: number | null;",
        "durationMs: number | null;",
        "sessionId: string | null;",
    ] {
        assert!(
            content.contains(field),
            "turnDone camelCase 字段缺失: {field}"
        );
    }
    // AgentBlock 四变体同为 kind 判别 + camelCase 字面量
    for literal in [
        "kind: \"text\"",
        "kind: \"thinking\"",
        "kind: \"toolUse\"",
        "kind: \"toolResult\"",
    ] {
        assert!(
            content.contains(literal),
            "AgentBlock 变体字面量缺失: {literal}"
        );
    }
}

#[test]
fn json_value字段出线unknown且时间戳字段出线string() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // serde_json::Value 字段统一出线 `unknown`（语义规则改写，与既有 dto.ts
    // payload / usage / input / key / value 口径一致，前端 renderer 自行收窄）
    for field in [
        "payload: unknown,",
        "usage: unknown;", // turnDone 内联变体，分号分隔
        "input: unknown",  // union 变体内联形态，无尾分隔符
        "key: unknown,",
        "value: unknown,",
    ] {
        assert!(content.contains(field), "Value 字段应出线 unknown: {field}");
    }

    // OffsetDateTime 字段出线 `string`（created / startAt 等，ISO-8601 文本）
    for field in ["created: string | null,", "startAt: string | null,"] {
        assert!(content.contains(field), "时间字段应出线 string: {field}");
    }
    assert!(
        !content.contains("OffsetDateTime"),
        "产物不含宿主类型名（出线口径为 TS 原生 string）"
    );

    // detail DTO 纯 derive（零 alias / skip_serializing_if / 自定义编解码），
    // specta phases 模式不得分裂出相位伴生类型（golden 契约的线面单一形态）
    assert!(
        !content.contains("_Deserialize"),
        "产物不得含 phases 分裂伴生类型（字段级 serde 属性会触发相位差）"
    );
}

// ---------------------------------------------------------------------------
// 导出幂等 / 过期产物纠正 / 目标目录缺失（AC-4）
// ---------------------------------------------------------------------------

#[test]
fn 同输入连续两次导出产物逐字节一致且无机器路径嵌入() {
    let _lock = lock();

    let first = authoritative_snapshot();
    export_bindings().expect("第二次导出应成功");
    let second = fs::read(generated_path()).expect("读取产物文件应成功");

    assert_eq!(
        first, second,
        "连续两次导出逐字节一致（确定性输出：无时间戳嵌入，AC-4 幂等判据）"
    );
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let content = String::from_utf8(second).expect("产物为 UTF-8 文本");
    assert!(
        !content.contains(manifest_dir),
        "产物不嵌入机器绝对路径，实际含 {manifest_dir}"
    );
}

#[test]
fn 预先篡改产物文件后重导出恢复权威内容() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let authoritative = authoritative_snapshot();

    // 模拟「改了 Rust 类型未重导出就提交」的过期入库物
    fs::write(generated_path(), "// 手改过的过期产物\n").expect("篡改产物应成功");
    assert_ne!(fs::read(generated_path()).unwrap(), authoritative);

    // 重导出即纠正：文件恢复为权威内容
    export_bindings().expect("重导出应成功");
    let restored = fs::read(generated_path()).expect("读取产物文件应成功");
    assert_eq!(
        restored, authoritative,
        "过期产物被重导出暴露并纠正（AC-4 机械半边）"
    );
}

#[test]
fn 产物父目录缺失时create_dir_all先行导出成功() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let authoritative = authoritative_snapshot();
    let dir = generated_path()
        .parent()
        .expect("产物应有父目录")
        .to_path_buf();

    // 整个生成目录被删（新克隆 / 清理后首启场景）：导出侧先建目录
    fs::remove_dir_all(&dir).expect("删除生成目录应成功");
    assert!(!dir.exists(), "前置：目录已不存在");

    export_bindings().expect("目录缺失时导出应成功（create_dir_all 先行）");
    assert!(dir.is_dir(), "父目录被重建");
    assert_eq!(
        fs::read(generated_path()).expect("产物应恢复"),
        authoritative,
        "重建产物与权威内容一致"
    );
}

// ---------------------------------------------------------------------------
// code_stats 三面 DTO 包装形态（23 条扩面，AC-2）
// ---------------------------------------------------------------------------

#[test]
fn code_stats绑定为root_depth入参的三面dto直返() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 包装形态逐字：camelCase 包装名 + (root: string, depth: number) 入参 +
    // `__TAURI_INVOKE<CodeStatsReport>` 直返（Throw 模式 Promise，错误面 reject）
    assert!(
        content.contains(
            "codeStats: (root: string, depth: number) => __TAURI_INVOKE<CodeStatsReport>(\"code_stats\", { root, depth })",
        ),
        "codeStats 包装形态不符（入参 / 返回类型 / invoke 命令名）"
    );
    // 三面 DTO 系类型 camelCase TS 镜像出线（share 上游怪癖出线 `number | null`，
    // 前端以 `share ?? 0` 防御）
    assert!(content.contains("export type CodeStatsReport = {"));
    assert!(content.contains("export type CodeTotals = {"));
    assert!(content.contains("export type LanguageStats = {"));
    assert!(content.contains("export type DirNode = {"));
    assert!(content.contains("export type FileNode = {"));
    // 树条目信封：tag `kind` 双变体（目录 / 文件叶），前端以 kind 收窄行形态
    assert!(content.contains("export type TreeEntry ="));
    assert!(content.contains("{ kind: \"dir\"; node: DirNode }"));
    assert!(content.contains("{ kind: \"file\"; node: FileNode }"));
    assert!(
        content.contains("share: number | null,"),
        "LanguageStats.share 出线为 `number | null`（specta 裸 f64 口径留档）"
    );
}

// ---------------------------------------------------------------------------
// Result 错误通道形态（Throw 模式：Err(String) → Promise reject 透传）
// ---------------------------------------------------------------------------

#[test]
fn 命令错误面为promise_reject透传无result包装() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // ErrorHandlingMode::Throw：生成包装直返 `__TAURI_INVOKE<T>` 的 Promise，
    // Err 经 reject 抵达前端（前端 hook 既有 `.catch → error 态` 接线不变）；
    // agent_stop 会话域寻址（root + sessionId）
    assert!(
        content.contains(
            "agentStop: (root: string, sessionId: string) => __TAURI_INVOKE<null>(\"agent_stop\", { root, sessionId })"
        ),
        "命令包装直返 invoke Promise（错误面 reject 透传）"
    );
    assert!(
        !content.contains("Result<"),
        "产物无 Result 包装（Throw 模式，非 Result 错误对象形态）"
    );
}

// ---------------------------------------------------------------------------
// workspace_config 配置域出线形态（AC-3）
// ---------------------------------------------------------------------------

#[test]
fn workspace_config绑定为root入参的report直返() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 包装形态逐字：camelCase 包装名 + (root: string) 入参 +
    // `__TAURI_INVOKE<WorkspaceConfigReport>` 直返（Throw 模式 Promise，
    // 错误面 reject 语义、无 Result 包装）
    assert!(
        content.contains(
            "workspaceConfig: (root: string) => __TAURI_INVOKE<WorkspaceConfigReport>(\"workspace_config\", { root })"
        ),
        "workspaceConfig 包装形态不符（入参 / 返回类型 / invoke 命令名）"
    );
    assert!(
        content.contains("export type WorkspaceConfigReport = {"),
        "WorkspaceConfigReport DTO 应出线"
    );
}

#[test]
fn 配置域十二型出线且逐字段形态对位() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // WorkspaceConfigReport：信封双字段（config + diagnostics）
    let report = type_section(&content, "WorkspaceConfigReport");
    assert!(report.contains("config: WorkspaceConfig,"), "实际 {report}");
    assert!(report.contains("diagnostics: ConfigDiagnostic[],"));

    // WorkspaceConfig：系 camelCase 类型镜像（$schema 线面字段名保留）
    let config = type_section(&content, "WorkspaceConfig");
    for field in [
        "$schema: string | null,",
        "schema: string,",
        "context: string | null,",
        "rules: RulesConfig | null,",
        "staticAnalysis: string | null,",
        "tests: TestSuite[],",
        "writeProtection: WriteProtection | null,",
        "extra: ConfigExtraField[],",
    ] {
        assert!(config.contains(field), "WorkspaceConfig 缺字段出线 {field}");
    }

    // TestSuite：全字段 camelCase 镜像
    let suite = type_section(&content, "TestSuite");
    for field in [
        "root: string,",
        "framework: TestFramework,",
        "cwd: string,",
        "config: string | null,",
        "includes: string[] | null,",
        "excludes: string[] | null,",
        "coverage: CoverageThresholds,",
        "mutation: MutationConfig,",
    ] {
        assert!(suite.contains(field), "TestSuite 缺字段出线 {field}");
    }

    // CoverageThresholds / MutationConfig：裸 f64 出线 `number | null` 口径
    let thresholds = type_section(&content, "CoverageThresholds");
    for field in [
        "lines: number | null,",
        "branches: number | null,",
        "functions: number | null,",
    ] {
        assert!(thresholds.contains(field), "CoverageThresholds 缺 {field}");
    }
    let mutation = type_section(&content, "MutationConfig");
    assert!(mutation.contains("cwd: string | null,"));
    assert!(mutation.contains("score: number | null,"));

    // RulesConfig / WriteProtection / WriteProtectionFile
    let rules = type_section(&content, "RulesConfig");
    assert!(rules.contains("proposal: string[] | null,"));
    assert!(rules.contains("tasks: string[] | null,"));
    let protection = type_section(&content, "WriteProtection");
    assert!(protection.contains("files: WriteProtectionFile[] | null,"));
    let file_rule = type_section(&content, "WriteProtectionFile");
    assert!(file_rule.contains("glob: string | null,"));
    assert!(file_rule.contains("reason: string | null,"));

    // ConfigExtraField / ConfigDiagnostic
    let extra = type_section(&content, "ConfigExtraField");
    assert!(extra.contains("key: string,"));
    assert!(
        extra.contains("value: unknown,"),
        "serde_json::Value 出线 unknown（语义规则既有口径回归）"
    );
    let diagnostic = type_section(&content, "ConfigDiagnostic");
    assert!(diagnostic.contains("kind: DiagnosticKind,"));
    assert!(diagnostic.contains("path: string,"));
    assert!(diagnostic.contains("message: string,"));
}

// ---------------------------------------------------------------------------
// session_detail 单查命令出线形态（desktop-change-session-visibility，AC-2 /
// AC-6 bindings 再生成半边）
// ---------------------------------------------------------------------------

#[test]
fn session_detail绑定为root_session_id入参的nullable三件套直返() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 包装形态逐字：camelCase 包装名 + (root: string, sessionId: string) 入参 +
    // 三件套内联返回 `| null`（blank root 空结果 Ok(None) 透传）+ invoke 命令名
    assert!(
        content.contains("sessionDetail: (root: string, sessionId: string) => __TAURI_INVOKE<{"),
        "sessionDetail 包装形态不符（入参 / 返回三件套内联形态）"
    );
    assert!(
        content.contains("} | null>(\"session_detail\", { root, sessionId })"),
        "session_detail invoke 目标与 nullable 空结果形态"
    );
    // 三件套字段出线（SessionSummary 复用零新 DTO——row / stats / turns）
    assert!(content.contains("row: SessionRow,"));
    assert!(content.contains("stats: SessionStats,"));
    assert!(content.contains("turns: TurnSummary[],"));
    // 同步直查命令：出线形态与 codeStats / workspaceConfig 同族直返（Throw 模式
    // Promise，错误面 reject 透传，无 Result 包装）
    assert!(
        !content.contains("Result<"),
        "产物无 Result 包装（Throw 模式回归）"
    );
}

#[test]
fn attempt_record出线三会话槽位键恒在场且nullable() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // AttemptRecord 类型名已在 DTO_TYPES 在册（本变更零补录）：三槽位字段
    // camelCase 出线、恒在场、无槽位 null（wire「缺省字段 null 不省键」契约面，
    // golden diff 守卫的字段级承载）
    let attempt = type_section(&content, "AttemptRecord");
    for field in [
        "executorSessionId: string | null,",
        "evaluatorSessionId: string | null,",
        "decisionSessionId: string | null,",
    ] {
        assert!(
            attempt.contains(field),
            "AttemptRecord 缺槽位出线 {field}，实际: {attempt}"
        );
    }
}

#[test]
fn framework与diagnostic_kind出线为camel_case字面量联合() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // TestFramework：八字符串字面量联合（非 rename_all 可表达的 vite-plus /
    // node-test 在册）
    let framework = type_section(&content, "TestFramework");
    for literal in [
        "\"jest\"",
        "\"vitest\"",
        "\"vite-plus\"",
        "\"bun\"",
        "\"rust\"",
        "\"node-test\"",
        "\"go\"",
        "\"pytest\"",
    ] {
        assert!(
            framework.contains(literal),
            "TestFramework 缺字面量 {literal}"
        );
    }
    assert_eq!(
        framework.matches('|').count(),
        7,
        "八值恰七分隔（不多不少，枚举 +1 侧不外溢）"
    );

    // DiagnosticKind：五值 camelCase 字面量联合（前端状态面映射口径）
    let kind = type_section(&content, "DiagnosticKind");
    for literal in [
        "\"fileMissing\"",
        "\"readFailed\"",
        "\"jsonInvalid\"",
        "\"invalidValue\"",
        "\"defaultApplied\"",
    ] {
        assert!(kind.contains(literal), "DiagnosticKind 缺字面量 {literal}");
    }
}

// ---------------------------------------------------------------------------
// 退役类型出线收缩（desktop-workflow-db-state，AC-8 / AC-9 后端半边随动）：
// `Inventory` / `FileLogEntry` / `FileLogOp` 镜像类型随 `parse/` 退役从产物
// 消失，`ChangeDetail` 三字段（inventory / unparsable / fileLog）删除、状态面
// 两键（status / created）新增——清单与产物集重新一致
// ---------------------------------------------------------------------------

#[test]
fn 退役类型inventory与filelog族从产物收缩_detail三字段删除状态面两键新增() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 产物文本不含退役类型名（bindings:export 再生后的直接证据）
    for retired in ["Inventory", "FileLogEntry", "FileLogOp"] {
        assert!(
            !content.contains(retired),
            "产物不应含已退役的 {retired} 类型名"
        );
    }

    // getChangeDetail 返回面无 inventory / unparsable / fileLog 三键：
    // ChangeDetail 类型段逐字段清点
    let detail = type_section(&content, "ChangeDetail");
    for retired_key in ["inventory", "unparsable", "fileLog"] {
        assert!(
            !detail.contains(retired_key),
            "ChangeDetail 出线面不应含已退役字段 {retired_key}，实际: {detail}"
        );
    }
    // 收敛后的字段集在场：三字段删除 + 状态面两键新增（字段面演进由生成物
    // 幂等重导 + golden diff 守卫承载，此处为出线形态对位）
    for field in [
        "name: string,",
        "source: ChangeSource,",
        "status: ChangeStatus | null,",
        "created: string | null,",
        "pipeline: PhaseEntry[],",
        "activePhase: ActivePhase | null,",
        "artifacts: ArtifactDescriptor[],",
    ] {
        assert!(detail.contains(field), "ChangeDetail 缺字段出线 {field}");
    }
}

#[test]
fn dto_types清单不残留退役幽灵条目() {
    // 门禁语义回归：DTO_TYPES 与产物集强耦合（产物缺出线类型 panic）——退役
    // 类型移除后清单必须不残留对应条目，否则覆盖性用例在产物侧命中不到类型
    // 声明即挂（删类型后不随动必挂的反向证明即本断言恢复绿）
    for retired in ["InterruptedEntry", "Inventory", "FileLogEntry", "FileLogOp"] {
        assert!(
            !DTO_TYPES.contains(&retired),
            "DTO_TYPES 清单不应残留已退役的 {retired} 条目"
        );
    }
    // 清单与产物集重新一致：既有覆盖性用例所遍历的每个类型名都能在产物命中
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    for dto in DTO_TYPES {
        assert!(
            content.contains(&format!("export type {dto} =")),
            "清单条目 {dto} 在产物缺出线类型声明（清单与产物集漂移）"
        );
    }
}

#[test]
fn command清单补录_archive_change后32条且既有锚点与在册命令保持() {
    // 本变更自身新命令补录（历史缺录为既有滞后债，contains 语义不致红、不在
    // 本变更范围）：条目数 31 → 32，首尾锚点不变
    assert_eq!(COMMAND_NAMES.len(), 32, "命令清单恰补录一条");
    assert_eq!(COMMAND_WRAPPERS.len(), 32, "包装清单恰补录一条");
    assert_eq!(COMMAND_NAMES.first(), Some(&"list_changes"));
    assert_eq!(COMMAND_NAMES.last(), Some(&"workspace_config"));
    assert!(COMMAND_NAMES.contains(&"archive_change"), "invoke 名补录");
    assert!(COMMAND_WRAPPERS.contains(&"archiveChange"), "包装名补录");
    // 无 interrupted 相关命令混入（历史口径持衡）
    assert!(
        COMMAND_NAMES
            .iter()
            .all(|name| !name.contains("interrupted")),
        "命令清单不应含 interrupted 相关命令"
    );
}

// ---------------------------------------------------------------------------
// archive_change 与新 DTO 出线形态（desktop-workflow-db-state，AC-8 bindings
// 出线半边）：新命令包装 + ArchiveOutcome / ChangeStatus 两类型
// ---------------------------------------------------------------------------

#[test]
fn archive_change绑定为root_change入参的outcome直返() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 包装形态逐字：camelCase 包装名 + (root: string, change: string) 入参 +
    // `__TAURI_INVOKE<ArchiveOutcome>` 直返（Throw 模式 Promise，错误面 reject
    // 透传——D11 无 UI 入口的 IPC 薄命令出线）
    assert!(
        content.contains(
            "archiveChange: (root: string, change: string) => __TAURI_INVOKE<ArchiveOutcome>(\"archive_change\", { root, change })",
        ),
        "archiveChange 包装形态不符（入参 / 返回类型 / invoke 命令名）"
    );
}

#[test]
fn archive_outcome与change_status出线且change_status小写线值联合() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // ArchiveOutcome：双字段 camelCase 出线（name + archivedDate）
    let outcome = type_section(&content, "ArchiveOutcome");
    assert!(outcome.contains("name: string,"), "实际: {outcome}");
    assert!(outcome.contains("archivedDate: string,"), "实际: {outcome}");

    // ChangeStatus：二值小写线格式联合（active / archived——建档判别面线词）
    let status = type_section(&content, "ChangeStatus");
    assert!(status.contains("\"active\""), "实际: {status}");
    assert!(status.contains("\"archived\""), "实际: {status}");
    assert_eq!(
        status.matches('|').count(),
        1,
        "二值恰一分隔（不多不少，枚举 +1 侧不外溢）"
    );
}

#[test]
fn 在册类型不重录_既有五型各恰一条() {
    // ChangeDetail / ChangeSummary / ActivePhase / AttemptRecord / ChecklistItem
    // 已在册类型不重复补录：字段面演进由生成物幂等重导 + golden diff 守卫承载
    //（核实结论行——清单只做类型名集合，零重复条目）
    for existing in [
        "ChangeDetail",
        "ChangeSummary",
        "ActivePhase",
        "AttemptRecord",
        "ChecklistItem",
    ] {
        assert_eq!(
            DTO_TYPES.iter().filter(|dto| **dto == existing).count(),
            1,
            "在册类型 {existing} 应恰一条（不重录）"
        );
    }
}

// ---------------------------------------------------------------------------
// worktree 维度 DTO 出线（design D2 / D14 / AC-9）：CreateOutcome 四字段 +
// ChangeDetail.worktree
// ---------------------------------------------------------------------------

/// `CreateOutcome` 类型段恰 `name` / `created` / `worktree` / `warnings` 四
/// 字段（`worktree: string`、`warnings: string[]`）——无主仓 openspec 目录
/// 树路径字段（D2 恰四字段面）。
#[test]
fn create_outcome出线恰四字段_worktree与warnings类型对位() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let outcome = type_section(&content, "CreateOutcome");

    for field in ["name", "created", "worktree", "warnings"] {
        assert!(
            outcome.contains(&format!(" {field}: ")) || outcome.contains(&format!("\t{field}: ")),
            "CreateOutcome 出线字段 {field} 缺席，实际:\n{outcome}"
        );
    }
    assert!(
        outcome.contains("worktree: string"),
        "worktree 出线为非空 string（执行锚恒在场），实际:\n{outcome}"
    );
    assert!(
        outcome.contains("warnings: string[]"),
        "warnings 出线为 string[]（清单恒在场空不省键），实际:\n{outcome}"
    );
    // 恰四字段：类型段内字段声明恰 4 行
    let field_count = outcome
        .lines()
        .filter(|line| {
            line.contains(":")
                && !line.trim_start().starts_with('*')
                && !line.trim_start().starts_with('/')
        })
        .filter(|line| {
            line.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
        .filter(|line| line.contains("string") || line.contains("[]"))
        .count();
    assert_eq!(
        field_count, 4,
        "恰四字段面（无主仓 openspec 目录树路径字段），实际:\n{outcome}"
    );
}

/// `ChangeDetail` 类型段含 `worktree: string | null`（None → null 出线——
/// 类型面与 golden 面双锚）。
#[test]
fn change_detail出线含worktree_string_or_null() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let detail = type_section(&content, "ChangeDetail");

    assert!(
        detail.contains("worktree: string | null"),
        "ChangeDetail 含 worktree: string | null（legacy null 出线），实际:\n{detail}"
    );
}

/// 签面不变：`create_change` 命令包装参数面不变（root / name / goal——async
/// 化零 bindings 漂移）。
#[test]
fn create_change命令签面_root_name_goal三参不变() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 命令包装形态：invoke 目标 + 三参对象（async 化零漂移）
    assert!(
        content.contains("createChange: (root: string, name: string, goal: string) =>"),
        "createChange 参数面不变（root / name / goal），实际段缺失"
    );
    assert!(
        content.contains(r#"__TAURI_INVOKE<CreateOutcome>("create_change", { root, name, goal })"#),
        "invoke 目标 create_change 与三参对象不变"
    );
}
