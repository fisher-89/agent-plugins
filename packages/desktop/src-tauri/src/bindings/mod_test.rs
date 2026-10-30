use std::fs;
use std::path::PathBuf;

use crate::bindings::export_bindings;

/// 产物文件路径（与 `export_bindings` 内 `CARGO_MANIFEST_DIR` 定位同式）。
fn generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 应有父目录")
        .join("src/types/generated/bindings.ts")
}

/// dto.ts 过渡 shim 路径（同式定位）。shim 为 `export type *` 星号 re-export
/// 生成绑定——出线 DTO（含 `RecordsPage`）经此自动跟随，禁手写旁路。
fn dto_shim_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 应有父目录")
        .join("src/types/dto.ts")
}

/// 产物文件互斥（**跨进程**）：单一共享产物（`src/types/generated/`）由多个
/// 测试独占改写（篡改 → 重导出 → 比对），必须串行。
///
/// 用 OS 级文件锁实现而非进程内 `Mutex`：测试框架已切到 nextest，每个测试跑在
/// **独立进程**里，进程内互斥对它完全无效——两个用例会同时改写同一份产物并互
/// 判定为「篡改未恢复」。
///
/// 为什么不再用「独占创建 + `Drop` 删文件」：Windows 上文件删除后、句柄关闭前
/// 处于 delete-pending 状态，此时另一进程 `create_new` 拿到的是
/// `ERROR_ACCESS_DENIED(5)` 而不是 `ERROR_FILE_EXISTS(80)`，落进兜底 panic 臂
/// （现网实测：`创建产物文件锁失败（…bindings-test.lock）: 拒绝访问。 (os error
/// 5)`）。更糟的是持锁期间 `remove_file` 本身会成功（共享模式含 `FILE_SHARE_DELETE`），
/// 「陈旧锁夺取」能让两个进程各持一个**不同 inode** 的锁、互不互斥地双写产物。
///
/// OS 文件锁没有这两面：锁随句柄关闭自动释放（进程崩溃不会留残留），
/// 文件系统层保证互斥（不会出现两个 inode）。锁文件本身常驻不删——删除只在
/// 「共享模式下临时还成功」时才会发生，而常驻文件从根上排除该窗口。
fn lock_file_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("bindings-test.lock")
}

/// 等锁上限：正常用例在秒级完成；超时说明有别的持锁进程卡死（不是死锁自愈
/// 面，直接失败更有诊断价值）。
const LOCK_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// 轮询间隔（`try_lock` 非阻塞，轮询而非阻塞等待以便观测超时）。
const LOCK_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(20);

/// 跨进程互斥守卫：持锁期间独占产物目录。释放靠 `File` 句柄关闭——`Drop` 无需
/// 删除文件，因此不存在 delete-pending 窗口。
struct FileLock(#[allow(dead_code)] fs::File);

fn lock() -> FileLock {
    let path = lock_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // 打开（不 `create_new`、不截断）：文件常驻，锁语义全在 OS 锁上
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .unwrap_or_else(|error| panic!("打开产物文件锁失败（{}）: {error}", path.display()));
    let deadline = std::time::Instant::now() + LOCK_WAIT_TIMEOUT;
    loop {
        match file.try_lock() {
            Ok(()) => return FileLock(file),
            // 锁被别的句柄 / 进程持有：继续轮询（含同进程的第二个句柄——本函数
            // 在每个用例里各调一次，跨进程是主场景）
            Err(fs::TryLockError::WouldBlock) => {
                if std::time::Instant::now() > deadline {
                    panic!("等待产物文件锁超时: {}", path.display());
                }
                std::thread::sleep(LOCK_POLL_INTERVAL);
            }
            Err(fs::TryLockError::Error(error)) => {
                panic!("创建产物文件锁失败（{}）: {error}", path.display())
            }
        }
    }
}

/// 权威产物快照：重导出（幂等确定性）后读取字节。
fn authoritative_snapshot() -> Vec<u8> {
    export_bindings().expect("export_bindings 应成功");
    fs::read(generated_path()).expect("读取产物文件应成功")
}

/// AC-8 自检桩路径（**手写**文件，非产物：目录整体删除后不会被重导出重建）。
fn zod_check_stub_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 应有父目录")
        .join("src/types/generated/bindings.zod.check.ts")
}

/// 变更类用例的 Drop 守卫：断言失败 unwind 时尽力重导出恢复权威产物
/// （仅在持有 [`file_lock`] 的用例内使用，不重复加锁）。
struct RestoreOnDrop;

/// 目录整体删除类用例的补位守卫：自检桩是手写文件（导出不会重建它，而
/// `bindings:zod-check` 依赖它在场），删除目录前先快照、drop 时写回——
/// 正常返回与 unwind 两条路径都不留缺口。
struct RestoreStubOnDrop(Vec<u8>);

impl Drop for RestoreStubOnDrop {
    fn drop(&mut self) {
        let _ = fs::write(zod_check_stub_path(), &self.0);
    }
}

impl Drop for RestoreOnDrop {
    fn drop(&mut self) {
        let _ = export_bindings();
    }
}

/// 38 条命令的生成包装名（camelCase，与 `generate_handler!` 时代命令清单一一
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
    "listChangeChatSessions",
    "listArchiChatSessions",
    "readExplore",
    "scanExplores",
    "exploreDocPath",
    "listExploreRecords",
    "createExploreRecord",
    "renameExploreRecord",
    "deleteExploreRecord",
    "updateExploreTitle",
    "updateExploreWorkflowType",
    "ensureExploreSession",
    "promoteExplore",
    "watchSubscribe",
    "watchUnsubscribe",
    "dbModels",
    "dbRecords",
    "codeStats",
    "workspaceConfig",
    "appEnvironment",
    "archiGraph",
    "archiModule",
    "archiReadDoc",
    "archiChangeSummary",
    "archiSetGate",
    "archiDrift",
];

/// 38 条命令的 IPC 命令名（snake_case，invoke 目标；补录口径同上）。
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
    "list_change_chat_sessions",
    "list_archi_chat_sessions",
    "read_explore",
    "scan_explores",
    "explore_doc_path",
    "list_explore_records",
    "create_explore_record",
    "rename_explore_record",
    "delete_explore_record",
    "update_explore_title",
    "update_explore_workflow_type",
    "ensure_explore_session",
    "promote_explore",
    "watch_subscribe",
    "watch_unsubscribe",
    "db_models",
    "db_records",
    "code_stats",
    "workspace_config",
    "app_environment",
    "archi_graph",
    "archi_module",
    "archi_read_doc",
    "archi_change_summary",
    "archi_set_gate",
    "archi_drift",
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
    "SessionPage",
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
    "ProjectConfig",
    "PromoteOutcome",
    "RecordEnvelope",
    "RecordsPage",
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
    "AppEnvironment",
    // archi 章节（架构模型纯读域）出线类型
    "ArchiChangeSummary",
    "ArchiCheckStatus",
    "ArchiDependedBy",
    "ArchiDependsOn",
    "ArchiDiagnostic",
    "ArchiDiagnosticKind",
    "ArchiDriftEntry",
    "ArchiDriftKind",
    "ArchiDriftMode",
    "ArchiDriftModel",
    "ArchiDriftObservation",
    "ArchiDriftOccurrence",
    "ArchiDriftReport",
    "ArchiDriftSuppressedEntry",
    "ArchiElement",
    "ArchiExcludedFile",
    "ArchiGateChange",
    "ArchiGateCount",
    "ArchiGraph",
    "ArchiMatchedElement",
    "ArchiModuleDetail",
    "ArchiObservationKind",
    "ArchiRelation",
    "ArchiScanReport",
    "ArchiSuppressVia",
    "ArchiUnresolvedFile",
    "ArchiWarning",
    "DocKind",
    "DocRef",
    "DocSource",
    "EdgeMark",
    "ElementKind",
    "GateTier",
    "NodeMark",
    "WarningKind",
];

/// 类型段内逐字段名（顶层字段声明行：缩进后为标识符 + `:` 起头；doc 注释 /
/// 结构标记 / 空行不计）。顺序即出线序（specta 字段序 = 声明序）。
fn field_names(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if trimmed.is_empty()
                || trimmed.starts_with('*')
                || trimmed.starts_with('/')
                || trimmed.starts_with('}')
            {
                return None;
            }
            let name = trimmed.split(':').next()?.trim();
            let mut chars = name.chars();
            match chars.next() {
                Some(c) if c.is_ascii_alphabetic() || c == '_' => name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    .then(|| name.to_owned()),
                _ => None,
            }
        })
        .collect()
}

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

// ---------------------------------------------------------------------------
// 引擎侧会话注入词汇不出线 IPC 产物（反向锚定）
//
// 口径（与 test-design「`src/commands/mod.rs` -> `src/bindings/mod_test.rs`」
// 节一致）：`SessionInjections` / `SessionToolset` / `TurnQuestion` 是引擎侧会话
// 注入词汇（`crates/core/agent/src/runner.rs`，serde 线形由 engine crate 自持），
// **不属 IPC 面**——`all_commands!` / `collect_commands` 的出线参数与返回类型里
// 没有它，产物 `src/types/generated/bindings.ts` 全文零该类型名（specta 不出线
// = 预期形态，非缺漏）。
//
// 故本节不设「变体出线」正向断言（不虚构产物面），以后述反向锚定固定口径：
// 引擎侧词汇一旦被误接到 IPC 出线面即红；同源语义（4 组合 round-trip / 旧线
// 兼容 / 非法线值拒绝）由 `crates/core/agent/src/runner_test.rs` 的
// `SessionToolset::Archi` serde 用例承载。功能代码零改动。
// ---------------------------------------------------------------------------

#[test]
fn 引擎侧会话注入词汇不出线ipc产物() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for engine_side in ["SessionInjections", "SessionToolset", "TurnQuestion"] {
        assert!(
            !content.contains(engine_side),
            "引擎侧会话词汇 {engine_side} 不得出线 IPC 产物（serde 面归 core/agent，\
             同源断言在 crates/core/agent/src/runner_test.rs）；若确需出线则须同改\
             test-design 与本节反向锚定"
        );
    }
    // 出线面确实在场（非空产物误判为「零命中」的对照臂）
    assert!(
        content.contains("export type ArchiModuleDetail ="),
        "产物含 archi 域出线类型（对照臂）"
    );
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
    let wrapper_head = "agentStart: (onEvent: Channel<AgentRunMessage>, root: string, prompt: string, permissionMode: AgentPermissionMode, sessionId: string | null, source: string | null, sourceRef: string | null, agent: number | null, cwd: string | null) => ";
    assert!(
        content.contains(wrapper_head),
        "agentStart 绑定签名以尾部 cwd（nullable string）收尾（typed Channel 首参保持，10 参同序）"
    );
    assert!(
        content.contains(
            "\"agent_start\", { onEvent, root, prompt, permissionMode, sessionId, source, sourceRef, agent, cwd }"
        ),
        "invoke 参数对象恒含 agent / cwd 键与 sessionId 键（会话域化 + 执行根参数面）"
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
fn save_agent_instance绑定签名不收窄且配置快照键经store出线() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // saveAgentInstance 包装签名逐字（AC-2 线值锚 / AC-5 参数类型不收窄：
    // id / providerId 恒 `number | null`、engine 恒 `AgentEngineKind` 双值
    // 类型——校验层拒绝、非类型层收窄）
    assert!(
        content.contains(
            "saveAgentInstance: (id: number | null, name: string, engine: AgentEngineKind, providerId: number | null) => __TAURI_INVOKE<AgentInstanceRecord>(\"save_agent_instance\", { id, name, engine, providerId })"
        ),
        "saveAgentInstance 参数类型不收窄、invoke 参数对象恒四键"
    );

    // SessionRow.configSnapshot 出线恒为 opaque `unknown`（快照 JSON 不透明、
    // 零手写类型旁路）——AC-7 新键 agentInstanceId 的线面锚在 store serde 侧
    //（model_test 承载 camelCase 与数值保真断言）
    assert!(
        content.contains("configSnapshot: unknown,"),
        "配置快照出线保持 unknown（不结构化展开）"
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
    let _stub_restore =
        RestoreStubOnDrop(fs::read(zod_check_stub_path()).expect("读取 AC-8 自检桩应成功"));
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
    // 双产物形态下同一单点仍生效：第二产物一并重建
    assert!(
        zod_generated_path().is_file(),
        "bindings.zod.ts 在目录重建后同批恢复"
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
fn 配置域十三型出线且逐字段形态对位() {
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
        "projects: ProjectConfig[],",
        "tests: TestSuite[],",
        "writeProtection: WriteProtection | null,",
        "extra: ConfigExtraField[],",
    ] {
        assert!(config.contains(field), "WorkspaceConfig 缺字段出线 {field}");
    }

    // ProjectConfig：项目根单字段
    let project = type_section(&content, "ProjectConfig");
    assert!(
        project.contains("root: string,"),
        "ProjectConfig 缺字段出线 root: string，实际 {project}"
    );

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
    // 具名 `SessionSummary | null` 返回（blank root 空结果 Ok(None) 透传）+ invoke
    // 命令名（三件套收为具名 DTO，类型面语义不变）
    assert!(
        content.contains(
            "sessionDetail: (root: string, sessionId: string) => __TAURI_INVOKE<SessionSummary | null>(\"session_detail\", { root, sessionId })"
        ),
        "sessionDetail 包装形态不符（入参 / 返回具名 SessionSummary | null）"
    );
    // 三件套字段出线（SessionSummary——row / stats / turns）
    let summary = type_section(&content, "SessionSummary");
    assert!(summary.contains("row: SessionRow,"), "实际: {summary}");
    assert!(summary.contains("stats: SessionStats,"), "实际: {summary}");
    assert!(summary.contains("turns: TurnSummary[],"), "实际: {summary}");
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
fn command清单补录_app_environment后38条且既有锚点与在册命令保持() {
    // 本变更自身新命令补录（历史缺录为既有滞后债，contains 语义不致红、不在
    // 本变更范围）：条目数 37 → 38（app_environment，登记序即 all_commands!
    // 末尾），首锚不变、尾锚随新命令后移
    // 本变更自身再补录一条（架构页会话清单）：条目数 43 → 44
    assert_eq!(
        COMMAND_NAMES.len(),
        45,
        "命令清单补录六条 archi 命令 + list_archi_chat_sessions（44 → 45）"
    );
    assert_eq!(
        COMMAND_WRAPPERS.len(),
        45,
        "包装清单补录六条 archi 命令 + listArchiChatSessions（44 → 45）"
    );
    assert_eq!(COMMAND_NAMES.first(), Some(&"list_changes"));
    assert_eq!(
        COMMAND_NAMES.last(),
        Some(&"archi_drift"),
        "尾锚随六条 archi 命令后移（archi_drift 尾部登记）"
    );
    assert!(
        COMMAND_NAMES.contains(&"app_environment"),
        "既有尾锚条目仍在册"
    );
    assert!(COMMAND_NAMES.contains(&"archive_change"), "invoke 名补录");
    assert!(COMMAND_WRAPPERS.contains(&"archiveChange"), "包装名补录");
    assert!(COMMAND_NAMES.contains(&"promote_explore"), "invoke 名补录");
    assert!(
        COMMAND_NAMES.contains(&"update_explore_title"),
        "invoke 名补录"
    );
    // 本变更新增命令（explore 会话归位唯一入口）
    assert!(
        COMMAND_NAMES.contains(&"ensure_explore_session"),
        "invoke 名补录（ensure_explore_session）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&"ensureExploreSession"),
        "包装名补录（ensureExploreSession）"
    );
    // 本变更新增命令（某 change 的 chat 会话清单薄包装）
    assert!(
        COMMAND_NAMES.contains(&"list_change_chat_sessions"),
        "invoke 名补录（list_change_chat_sessions）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&"listChangeChatSessions"),
        "包装名补录（listChangeChatSessions）"
    );
    // 前轮变更新增命令（explore 工作流类型回填唯一写口）——在册保持
    assert!(
        COMMAND_NAMES.contains(&"update_explore_workflow_type"),
        "invoke 名补录（update_explore_workflow_type）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&"updateExploreWorkflowType"),
        "包装名补录（updateExploreWorkflowType）"
    );
    // 无 interrupted 相关命令混入（历史口径持衡）
    assert!(
        COMMAND_NAMES
            .iter()
            .all(|name| !name.contains("interrupted")),
        "命令清单不应含 interrupted 相关命令"
    );
}

/// DTO 类型清单（AC-9 守卫半边）：本变更新增 `RecordsPage` 一个出线 DTO
/// 类型名（分页信封返回面）——清单条目数随动，首尾锚与在册条目保持。
#[test]
fn dto类型清单零补录_条目数与首尾锚持衡() {
    assert_eq!(
        DTO_TYPES.len(),
        94,
        "DTO 清单补录 19 条 archi 章节 + 16 条架构漂移出线类型 + `ProjectConfig`（77 + 17）"
    );
    assert_eq!(DTO_TYPES.first(), Some(&"ActivePhase"), "首锚持衡");
    assert_eq!(
        DTO_TYPES.last(),
        Some(&"WarningKind"),
        "尾锚随 archi 章节出线类型后移"
    );
    assert!(
        DTO_TYPES.contains(&"AppEnvironment"),
        "既有尾锚条目仍在册（不因追加丢失）"
    );
    assert!(
        DTO_TYPES.contains(&"WriteProtectionFile"),
        "既有尾锚条目仍在册（不因追加丢失）"
    );
}

// ---------------------------------------------------------------------------
// RecordsPage 出线形态（desktop-manage-db-by-table，AC-3 bindings 再生成半边）：
// 分页信封 DTO + dbRecords 返回类型 + DTO_TYPES 清单随动（DD6）
// ---------------------------------------------------------------------------

/// `RecordsPage` 类型段逐字段出线（`records: RecordEnvelope[]` + `total: number`，
/// 声明序即出线序）；`dbRecords` 包装返回 `__TAURI_INVOKE<RecordsPage>`（Throw
/// 模式 Promise，无 `Result` 包装）。
#[test]
fn records_page出线为records与total两字段且db_records绑定返回分页信封() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 类型段恰两字段且顺序在场（声明序 = 出线序，specta 字段序）
    let section = type_section(&content, "RecordsPage");
    assert_eq!(
        field_names(&section),
        vec!["records", "total"],
        "RecordsPage 恰 records / total 两字段且序不变，实际段:\n{section}"
    );
    assert!(
        section.contains("records: RecordEnvelope[],") && section.contains("total: number,"),
        "RecordsPage 字段类型出线（分页信封 + 全量计数），实际段:\n{section}"
    );

    // dbRecords 绑定返回类型升级为 RecordsPage，六参透传面不变（命令名与
    // 入参对象逐字随动生成，非手写旁路）
    assert!(
        content.contains(
            "dbRecords: (scope: DbDimension, root: string, model: string, offset: number, \
             limit: number) => __TAURI_INVOKE<RecordsPage>(\"db_records\", { scope, root, \
             model, offset, limit })"
        ),
        "dbRecords 绑定返回 __TAURI_INVOKE<RecordsPage>（入参面不变）"
    );
    // Throw 模式：产物无 Result 错误对象包装
    assert!(
        !content.contains("Result<"),
        "产物无 Result 包装（Throw 模式回归）"
    );

    // dto.ts 的 `export type *` re-export 自动跟随：`RecordsPage`（及全部出线
    // DTO）经星号 re-export 出线 shim，无需逐类型手写补录（shim MUST NOT 承载
    // 手写类型定义——禁手写旁路反向锚：shim 内不出现 RecordsPage 手写声明）
    let dto_shim = fs::read_to_string(dto_shim_path()).expect("读取 dto.ts 应成功");
    assert!(
        dto_shim.contains("export type * from './generated/bindings';"),
        "dto.ts 应以 `export type *` 星号 re-export 生成绑定（RecordsPage 自动跟随）"
    );
    assert!(
        !dto_shim.contains("RecordsPage"),
        "dto.ts 不应手写 RecordsPage（星号 re-export 自动跟随，禁手写旁路）"
    );
}

/// `SessionPage` 类型段逐字段出线（`sessions: SessionSummary[]` + `total: number`，
/// 声明序即出线序）；`agentSessions` 包装返回 `__TAURI_INVOKE<SessionPage>`，六参
/// 透传面携分页入参（root / source / sourceRef / offset / limit）。
#[test]
fn session_page出线为sessions与total两字段且agent_sessions绑定返回分页信封() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 类型段恰两字段且顺序在场（声明序 = 出线序，specta 字段序）
    let section = type_section(&content, "SessionPage");
    assert_eq!(
        field_names(&section),
        vec!["sessions", "total"],
        "SessionPage 恰 sessions / total 两字段且序不变，实际段:\n{section}"
    );
    assert!(
        section.contains("sessions: SessionSummary[],") && section.contains("total: number,"),
        "SessionPage 字段类型出线（分页信封 + 全量计数），实际段:\n{section}"
    );

    // agentSessions 绑定返回类型升级为 SessionPage，分页入参透传（非手写旁路）
    assert!(
        content.contains(
            "agentSessions: (root: string, source: string | null, sourceRef: string | null, \
             offset: number, limit: number) => __TAURI_INVOKE<SessionPage>(\"agent_sessions\", \
             { root, source, sourceRef, offset, limit })"
        ),
        "agentSessions 绑定返回 __TAURI_INVOKE<SessionPage>（分页入参透传）"
    );
    // listChangeChatSessions：某 change 的 chat 清单薄包装（包装名 / invoke 名 /
    // 返回 SessionPage 三面逐字）
    assert!(
        content.contains(
            "listChangeChatSessions: (root: string, id: string, offset: number, limit: number) => __TAURI_INVOKE<SessionPage>(\"list_change_chat_sessions\", { root, id, offset, limit })"
        ),
        "listChangeChatSessions 绑定返回 __TAURI_INVOKE<SessionPage>（root / id / offset / limit 四参透传）"
    );

    // dto.ts 星号 re-export 自动跟随（禁手写旁路反向锚）
    let dto_shim = fs::read_to_string(dto_shim_path()).expect("读取 dto.ts 应成功");
    assert!(
        !dto_shim.contains("SessionPage"),
        "dto.ts 不应手写 SessionPage（星号 re-export 自动跟随，禁手写旁路）"
    );
}

/// 清单与产物强耦合门禁（DD6 反向证明）：`"RecordsPage"` 在场后「产物缺出线
/// 类型」panic 断言不触发；清单无幽灵（在册条目皆出线）/ 无遗漏（出线 DTO 段
/// 皆在册）。
#[test]
fn dto清单与产物types段全集一致_无幽灵与遗漏条目() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 无遗漏：清单每条都能命中产物 `export type <名> =` 声明——「产物缺出线
    // 类型」panic 断言在 `"RecordsPage"` 在场后不触发（出线类型增删不随动清单
    // 必挂的反向证明恢复绿）
    for dto in DTO_TYPES {
        assert!(
            content.contains(&format!("export type {dto} =")),
            "清单条目 {dto} 在产物缺出线类型声明（清单与产物集漂移）"
        );
    }

    // 无幽灵：清单零重复条目（`RecordsPage` 恰一条，非重复补录）
    let records_pages = DTO_TYPES
        .iter()
        .filter(|dto| **dto == "RecordsPage")
        .count();
    assert_eq!(records_pages, 1, "RecordsPage 在清单恰一条（零重录幽灵）");
}

// ---------------------------------------------------------------------------
// archive_change 与新 DTO 出线形态（desktop-workflow-db-state，AC-8 bindings
// 出线半边）：新命令包装 + ArchiveOutcome / ChangeStatus 两类型
// ---------------------------------------------------------------------------

#[test]
fn archive_change绑定为root_id入参的outcome直返() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 包装形态逐字：camelCase 包装名 + (root: string, id: string) 入参 +
    // `__TAURI_INVOKE<ArchiveOutcome>` 直返（Throw 模式 Promise，错误面 reject
    // 透传——D11 无 UI 入口的 IPC 薄命令出线；定位参数 id 化——AC-5 签面随动）
    assert!(
        content.contains(
            "archiveChange: (root: string, id: string) => __TAURI_INVOKE<ArchiveOutcome>(\"archive_change\", { root, id })",
        ),
        "archiveChange 包装形态不符（id 入参 / 返回类型 / invoke 命令名）"
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
// 身份锚出线（desktop-change-db-identity D8 / AC-5）：CreateOutcome 恰五字段
// （id 首字段）+ ChangeDetail / ChangeSummary 含 id；worktree 维度字段面随行
// ---------------------------------------------------------------------------

/// `CreateOutcome` 类型段恰 `id` / `name` / `created` / `worktree` /
/// `warnings` 五字段且 `id` 为首字段（`id: string`、`worktree: string`、
/// `warnings: string[]`）——原恰四字段断言语义演进（增铸出 id 身份锚），无主仓
/// openspec 目录树路径字段。
#[test]
fn create_outcome出线恰五字段_id为首字段() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let outcome = type_section(&content, "CreateOutcome");

    assert_eq!(
        field_names(&outcome),
        vec!["id", "name", "created", "worktree", "warnings"],
        "恰五字段面且 id 为首字段（声明序即出线序），实际:\n{outcome}"
    );
    assert!(
        outcome.contains("id: string,"),
        "id 出线为 string（铸出身份锚恒在场），实际:\n{outcome}"
    );
    assert!(
        outcome.contains("worktree: string,"),
        "worktree 出线为非空 string（执行锚恒在场），实际:\n{outcome}"
    );
    assert!(
        outcome.contains("warnings: string[],"),
        "warnings 出线为 string[]（清单恒在场空不省键），实际:\n{outcome}"
    );
}

/// `ChangeDetail` / `ChangeSummary` 两类型段含 `id: string` 为首字段（出线
/// 增量最小化——D8：id 为既有 DTO 的字段增量，零新增类型名）。
#[test]
fn change_detail与change_summary出线含id字段为首() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for type_name in ["ChangeDetail", "ChangeSummary"] {
        let section = type_section(&content, type_name);
        assert!(
            section.contains("id: string,"),
            "{type_name} 段应含 id: string 字段，实际:\n{section}"
        );
        assert_eq!(
            field_names(&section).first().map(String::as_str),
            Some("id"),
            "{type_name} 的 id 应为首字段（身份锚出线序），实际:\n{section}"
        );
    }
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

/// 签面随动：`create_change` 命令包装参数面扩为四参（root / name / goal /
/// workflowType——desktop-test-only-flow 类型选择器入参）。
#[test]
fn create_change命令签面_root_name_goal_workflow_type四参() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 命令包装形态：invoke 目标 + 四参对象
    assert!(
        content.contains(
            "createChange: (root: string, name: string, goal: string, workflowType: string) =>"
        ),
        "createChange 参数面为四参（root / name / goal / workflowType），实际段缺失"
    );
    assert!(
        content.contains(
            r#"__TAURI_INVOKE<CreateOutcome>("create_change", { root, name, goal, workflowType })"#
        ),
        "invoke 目标 create_change 与四参对象随动"
    );
}

// ---------------------------------------------------------------------------
// 归档链退役（merge-archive-steps-into-flow AC-9 / AC-10）：归档五命令 /
// ArchiveUpdate 家族 / ArchiveStage 词汇随归档链整体退役——生成物不含退役命令
// 包装名与退役类型（bindings 零手改，再生成直出线；退役 diff 面由 code-review
// 与 bindings:check 管线承载）
// ---------------------------------------------------------------------------

/// 归档链退役面：生成 bindings 不含退役五命令包装名与退役 invoke 名（归档发起 /
/// 停止 / 状态 / 预检 / 观察全并入 `changeFlow*` 命令面——AC-10）。
#[test]
fn 归档五命令退役_生成物零残留() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for wrapper in [
        "archiveFlowPreflight",
        "archiveFlowStart",
        "archiveFlowStop",
        "archiveFlowState",
        "archiveFlowWatch",
    ] {
        assert!(
            !content.contains(&format!("{wrapper}:")),
            "生成物残留退役命令包装名 {wrapper}"
        );
    }
    for name in [
        "archive_flow_preflight",
        "archive_flow_start",
        "archive_flow_stop",
        "archive_flow_state",
        "archive_flow_watch",
    ] {
        assert!(
            !content.contains(&format!("\"{name}\"")),
            "生成物残留退役 invoke 命令名 {name}"
        );
    }
    // 退役类型面零残留（ArchiveStage / ArchiveUpdate / ArchiveSnapshot 等）
    for retired in [
        "ArchiveStage",
        "ArchiveStageStatus",
        "ArchiveSpecsStatus",
        "ArchiveUpdate",
        "ArchiveSnapshot",
        "ArchiveSummary",
        "ArchiveControl",
        "ArchiveSink",
    ] {
        assert!(!content.contains(retired), "生成物残留退役类型 {retired}");
    }
}

/// 归档前置载荷线面（正向）：归档链并入 run 后仅 `ArchivePreflight` 保留——八
/// 字段 camelCase 出线且经 `ChangeRunSnapshot.archive` / `ActiveRunView.archive`
/// 挂靠（AC-7 / AC-10 生成物面）。
#[test]
fn archive_preflight线面字段面保留() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let preflight = type_section(&content, "ArchivePreflight");
    for field in [
        "name",
        "completed",
        "incompletePhases",
        "missingArtifacts",
        "deltaSpecs",
        "worktree",
        "branch",
        "mergeTarget",
    ] {
        assert!(
            preflight.contains(&format!("{field}:")),
            "ArchivePreflight 字段 {field}"
        );
    }
    // 挂靠面：统一视图活面携 archive 槽（camelCase；快照为进程内 seam 类型，
    // 仅经 ActiveRunView 投影出线——`get_change_detail` 单一查询面）
    let active_run = type_section(&content, "ActiveRunView");
    assert!(
        active_run.contains("archive: ArchivePreflight | null"),
        "ActiveRunView 携 archive 槽: {active_run}"
    );
    // 归档相位停等经 changeFlow* 命令面承载（归档五命令零残留）
    assert!(
        content.contains("changeFlowStart: onEvent") || content.contains("changeFlowStart:"),
        "归档停等经 changeFlow* 命令面"
    );
    assert!(
        content.contains("changeFlowConfirm:"),
        "确认（含 syncSpecs）经 changeFlowConfirm 命令面"
    );
    assert!(
        !content.contains("archiveFlow"),
        "归档五命令零残留（生成物面）"
    );
}

// ---------------------------------------------------------------------------
// app_environment 登记面（desktop-build-mode-split，AC-14 / AC-16）：命令清单 +
// 生成物包装 + DTO 线面三字段。清单为产物子集的既有口径不变（历史缺录为既有
// 滞后债，contains 语义不致红），本变更只补录自身新命令。
// ---------------------------------------------------------------------------

/// 登记面正向：新命令进两份清单与 DTO 清单，既有 37 条逐条仍在（无重复登记）。
#[test]
fn 登记面_app_environment进两份命令清单且dto类型在册() {
    assert!(
        COMMAND_NAMES.contains(&"app_environment"),
        "invoke 命令名补录（app_environment）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&"appEnvironment"),
        "生成包装名补录（appEnvironment）"
    );
    assert!(
        DTO_TYPES.contains(&"AppEnvironment"),
        "出线 DTO 类型补录（AppEnvironment）"
    );

    // 既有在册命令逐条仍在（contains 语义，不因追加而丢）
    for name in [
        "list_changes",
        "archive_change",
        "ensure_explore_session",
        "list_change_chat_sessions",
        "workspace_config",
    ] {
        assert!(
            COMMAND_NAMES.contains(&name),
            "既有 invoke 名仍在册：{name}"
        );
    }

    // 无重复登记
    assert_eq!(
        COMMAND_NAMES
            .iter()
            .filter(|name| **name == "app_environment")
            .count(),
        1,
        "新命令恰登记一次"
    );
    assert_eq!(
        COMMAND_NAMES.len(),
        COMMAND_WRAPPERS.len(),
        "两份清单条目数持衡（一一对应）"
    );
}

/// 登记面生成物正向：包装形态逐字（camelCase 名 + `__TAURI_INVOKE<AppEnvironment>`
/// + snake_case invoke 目标）；DTO 恰三字段 camelCase 且字段序即声明序。
#[test]
fn 登记面_生成物含app_environment包装与线面三字段() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert!(
        content
            .contains("appEnvironment: () => __TAURI_INVOKE<AppEnvironment>(\"app_environment\")"),
        "包装形态不符（参数面 / 返回类型 / invoke 命令名）"
    );

    let section = type_section(&content, "AppEnvironment");
    assert!(
        section.contains("mode: string,"),
        "mode 字段在场（构建模式线值）"
    );
    assert!(
        section.contains("dataRoot: string,"),
        "dataRoot 字段 camelCase 出线"
    );
    assert!(
        section.contains("source: string,"),
        "source 字段在场（数据根来源线值）"
    );
    assert!(
        !section.contains("data_root:"),
        "无蛇形字段声明（serde rename_all 生效；doc 注释里的同源说明不计）"
    );
    assert_eq!(
        field_names(&section),
        vec!["mode", "dataRoot", "source"],
        "字段序即声明序（mode → dataRoot → source）"
    );
}

/// 登记面异常面：重复导出逐字节一致（`bindings:check` 的单测等价面——缺登记 /
/// 误改清单时该字节比对必红）。
#[test]
fn 登记面_重复导出逐字节一致且新包装在册() {
    let _lock = lock();

    let first = authoritative_snapshot();
    export_bindings().expect("第二次导出应成功");
    let second = fs::read(generated_path()).expect("读取产物文件应成功");

    assert_eq!(first, second, "两次导出逐字节一致（零 diff 门禁）");
    let content = String::from_utf8(second).expect("产物为 UTF-8 文本");
    assert!(content.contains("appEnvironment:"), "重导出后新包装仍在册");
    assert!(
        content.contains("export type AppEnvironment ="),
        "重导出后新 DTO 仍在册"
    );
}

/// 登记面边界：新命令为清单末条（其后即对象收尾 ⇒ 仅在末尾出现一次），且既有
/// 清单每条都能在产物命中（无遗漏；`all_commands!` 为唯一登记面）。
#[test]
fn 登记面_新命令为清单末条且既有清单零遗漏() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert_eq!(
        content.matches("appEnvironment:").count(),
        1,
        "包装在产物中恰一次（无重复登记）"
    );
    assert_eq!(
        content.matches("\"app_environment\"").count(),
        1,
        "invoke 目标在产物中恰一次"
    );

    // 清单末条随尾部追加随动：`app_environment` 之后追加了 archi 五命令，随后是本
    // 变更的 `archiDrift`（既有条目零位移——`appEnvironment` 仍在册且恰一次，见上）
    let index = content.find("archiDrift:").expect("包装在场");
    let tail = &content[index..];
    let line_end = tail.find('\n').expect("包装行有换行");
    assert!(
        tail[line_end + 1..].starts_with("};"),
        "新命令为清单末条（生成物中其后即对象收尾）"
    );

    // 清单是产物的子集：逐条命中（历史缺录的滞后债不在本变更范围）
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
}

// ---------------------------------------------------------------------------
// archi 章节：五命令与架构域出线类型在册（AC-20 的守卫面）
// ---------------------------------------------------------------------------

/// 架构域五命令（invoke 名 / 包装名成对）。
const ARCHI_COMMANDS: [(&str, &str); 5] = [
    ("archi_graph", "archiGraph"),
    ("archi_module", "archiModule"),
    ("archi_read_doc", "archiReadDoc"),
    ("archi_change_summary", "archiChangeSummary"),
    ("archi_set_gate", "archiSetGate"),
];

/// 架构域出线类型名（`Archi*` 投影 + 五组封闭词汇）。
const ARCHI_DTO_TYPES: [&str; 19] = [
    "ArchiChangeSummary",
    "ArchiDependedBy",
    "ArchiDependsOn",
    "ArchiDiagnostic",
    "ArchiDiagnosticKind",
    "ArchiElement",
    "ArchiGateChange",
    "ArchiGraph",
    "ArchiModuleDetail",
    "ArchiRelation",
    "ArchiWarning",
    "DocKind",
    "DocRef",
    "DocSource",
    "EdgeMark",
    "ElementKind",
    "GateTier",
    "NodeMark",
    "WarningKind",
];

/// 正向：五命令在册（清单 + 产物双面）；既有条目零位移（尾部追加）。
#[test]
fn archi五命令在册且既有条目零位移() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for (name, wrapper) in ARCHI_COMMANDS {
        assert!(
            COMMAND_NAMES.contains(&name),
            "命令清单应补录 invoke 名 {name}"
        );
        assert!(
            COMMAND_WRAPPERS.contains(&wrapper),
            "命令清单应补录包装名 {wrapper}"
        );
        assert!(
            content.contains(&format!("{wrapper}:")),
            "产物缺命令包装名 {wrapper}"
        );
        assert!(
            content.contains(&format!("\"{name}\"")),
            "产物缺 invoke 命令名 {name}"
        );
    }

    // 尾部追加：五条新命令位于 `app_environment` 之后且序固定
    let tail_start = COMMAND_NAMES
        .iter()
        .position(|name| *name == "app_environment")
        .expect("既有尾锚仍在册");
    let tail: Vec<&str> = COMMAND_NAMES[tail_start..].to_vec();
    assert_eq!(
        tail,
        vec![
            "app_environment",
            "archi_graph",
            "archi_module",
            "archi_read_doc",
            "archi_change_summary",
            "archi_set_gate",
            "archi_drift",
        ],
        "archi 命令尾部追加（既有条目零位移）"
    );
}

/// 正向：架构域出线类型逐名命中产物（`export type <Name> =`）。
#[test]
fn archi出线类型逐名命中产物() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    for dto in ARCHI_DTO_TYPES {
        assert!(DTO_TYPES.contains(&dto), "DTO_TYPES 清单应在册 {dto}");
        assert!(
            content.contains(&format!("export type {dto} =")),
            "产物缺出线 DTO 类型 {dto}"
        );
    }

    // `ArchiGraph` 三数组形状（停摆判据 = diagnostics 非空）与 `DocRef` 四字段
    let graph = type_section(&content, "ArchiGraph");
    assert_eq!(
        field_names(&graph),
        vec!["elements", "relations", "diagnostics"],
        "ArchiGraph 三字段按声明序出线（元素 / 关系 / 诊断），实际段:\n{graph}"
    );
    let doc_ref = type_section(&content, "DocRef");
    assert_eq!(
        field_names(&doc_ref),
        vec!["path", "title", "kind", "source"],
        "DocRef 恰四字段按声明序出线（无正文 / 无摘要），实际段:\n{doc_ref}"
    );
}

/// 边界：重导出幂等——连续两次 `export_bindings()` 产物逐字节一致（零 diff 门禁）；
/// 断言失败时 `RestoreOnDrop` 恢复产物。
#[test]
fn archi章节重导出幂等零diff() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let first = authoritative_snapshot();
    let second = authoritative_snapshot();
    assert_eq!(first, second, "连续两次重导出逐字节一致（零 diff）");
}

/// 异常（反向证明）：以本地清单副本删去一条新命令 → 产物覆盖断言必红（清单与
/// 产物强耦合的反向证据）。
#[test]
fn 清单遗漏即产物覆盖断言必红() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let mut copy: Vec<(&str, &str)> = ARCHI_COMMANDS.to_vec();
    let removed = copy.pop().expect("清单非空");
    assert!(
        content.contains(&format!("\"{}\"", removed.0)),
        "原清单条目在产物内（对照臂）"
    );
    // 反向臂：副本已不含该条目 → 「逐条覆盖」断言必然失败
    let missing: Vec<&str> = copy
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !content.contains(&format!("\"{name}\"")))
        .collect();
    assert!(
        missing.is_empty(),
        "覆盖断言口径下副本无缺失（{removed:?} 已被移出，故不参与）"
    );
    assert!(
        !copy.iter().any(|(name, _)| *name == removed.0),
        "副本确已删去 {removed:?}（清单改动即覆盖断言口径随动）"
    );
}

// ---------------------------------------------------------------------------
// 双产物守卫（AC-9）：`export_bindings` 是两份产物的唯一入口——`bindings.ts`
// （类型唯一源）与 `bindings.zod.ts`（纯 schema）同目录同批导出，任一失败即非零
// 退出；篡改 / 删除 / 目录缺失三面同样按权威内容恢复。串行靠既有 `file_lock()`，
// 变更类用例以 `RestoreOnDrop` 兜底恢复。
// ---------------------------------------------------------------------------

/// 第二产物路径（同式定位：`CARGO_MANIFEST_DIR` 的上级 + 生成目录）。
fn zod_generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 应有父目录")
        .join("src/types/generated/bindings.zod.ts")
}

/// 第二产物权威快照：重导出（幂等确定性）后读取字节。
fn zod_authoritative_snapshot() -> Vec<u8> {
    export_bindings().expect("export_bindings 应成功");
    fs::read(zod_generated_path()).expect("读取 zod 产物文件应成功")
}

#[test]
fn 双产物同目录同时存在且zod产物含三表与降级哨兵() {
    let _lock = lock();
    export_bindings().expect("导出应成功");

    assert!(generated_path().is_file(), "bindings.ts 应存在");
    assert!(zod_generated_path().is_file(), "bindings.zod.ts 应存在");
    assert_eq!(
        generated_path().parent(),
        zod_generated_path().parent(),
        "两份产物同目录（同一 create_dir_all 单点）"
    );

    let zod = String::from_utf8(fs::read(zod_generated_path()).expect("读取 zod 产物应成功"))
        .expect("zod 产物为 UTF-8 文本");
    // 三张表（表名即装配层取的三个导出）
    for table in ["argsSchemas", "resultSchemas", "channelArgs"] {
        assert!(
            zod.contains(&format!("export const {table} = {{")),
            "zod 产物缺 {table} 表"
        );
    }
    // 降级哨兵只落 5 个 Channel 形参位（agentStart / agentSessionWatch /
    // changeFlowStart / changeFlowWatch / watchSubscribe 的 onEvent）
    assert_eq!(
        zod.matches("degraded(").count(),
        5,
        "degraded( 恰 5 处（Channel 形参位）"
    );
    // 职责切分：schema 产物不改写命令面、不 import 类型产物
    assert!(
        !zod.contains("export const commands"),
        "zod 产物不含 commands 包装（职责切分）"
    );
    assert!(
        !zod.contains("from \"./bindings\"") && !zod.contains("from './bindings'"),
        "zod 产物不 import bindings.ts（零运行期耦合）"
    );
    // 生成期硬约束（AC-7）在真实产物上同样成立
    for snippet in [
        "export type ",
        "z.infer",
        "z.object(",
        ".refine(",
        ".default(",
    ] {
        assert!(!zod.contains(snippet), "zod 产物命中禁用片段 {snippet}");
    }
}

#[test]
fn 连续两次导出两份产物各自零diff() {
    let _lock = lock();

    let first_ts = authoritative_snapshot();
    let first_zod = fs::read(zod_generated_path()).expect("读取 zod 产物应成功");

    export_bindings().expect("第二次导出应成功");
    let second_ts = fs::read(generated_path()).expect("读取产物文件应成功");
    let second_zod = fs::read(zod_generated_path()).expect("读取 zod 产物应成功");

    assert_eq!(first_ts, second_ts, "bindings.ts 连续两次导出零 diff");
    assert_eq!(first_zod, second_zod, "bindings.zod.ts 连续两次导出零 diff");
}

#[test]
fn 删除zod产物后重导出重建为权威内容() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let authoritative = zod_authoritative_snapshot();
    let path = zod_generated_path();

    fs::remove_file(&path).expect("删除 zod 产物应成功");
    assert!(!path.exists(), "前置：第二产物已不存在");

    export_bindings().expect("重导出应成功");
    assert!(path.is_file(), "第二产物被重建");
    assert_eq!(
        fs::read(&path).expect("读取重建产物应成功"),
        authoritative,
        "重建内容与权威内容一致（第二产物同受守卫）"
    );
}

#[test]
fn 手工篡改zod产物后重导出恢复权威内容() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let authoritative = zod_authoritative_snapshot();
    let path = zod_generated_path();

    // 两类篡改：删掉一张表 / 注入 `export type`（类型别名是被 PATCH(2) 删除的形态）
    for tampered in [
        "// 手改：删掉了三张表\n",
        "export type HijackedSchema = unknown;\n",
    ] {
        fs::write(&path, tampered).expect("篡改 zod 产物应成功");
        assert_ne!(fs::read(&path).unwrap(), authoritative, "前置：篡改生效");

        export_bindings().expect("重导出应成功");
        assert_eq!(
            fs::read(&path).expect("读取产物应成功"),
            authoritative,
            "篡改不留存（重导出恢复权威内容）"
        );
    }
}

// ---------------------------------------------------------------------------
// 架构页会话清单命令（`list_archi_chat_sessions`）：`all_commands!` 唯一登记面 +
// 生成产物双面（漏登记则 IPC 与 `bindings.ts` 同时失守）。既有条目零位移——新命令
// 紧跟 `list_change_chat_sessions` 之后登记（同族相邻，非尾部追加）。
// ---------------------------------------------------------------------------

/// 新命令的 invoke 名 / 包装名 / 生成行（三面逐字锚）。
const ARCHI_CHAT_COMMAND: &str = "list_archi_chat_sessions";
const ARCHI_CHAT_WRAPPER: &str = "listArchiChatSessions";
const ARCHI_CHAT_WRAPPER_LINE: &str = "listArchiChatSessions: (root: string, offset: number, limit: number) => __TAURI_INVOKE<SessionPage>(\"list_archi_chat_sessions\", { root, offset, limit }),";

/// 正向：invoke 名与包装名双双在册（`COMMAND_NAMES` / `COMMAND_WRAPPERS`）。
#[test]
fn 登记面_架构页会话清单命令在册() {
    assert!(
        COMMAND_NAMES.contains(&ARCHI_CHAT_COMMAND),
        "invoke 命令名补录（{ARCHI_CHAT_COMMAND}）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&ARCHI_CHAT_WRAPPER),
        "生成包装名补录（{ARCHI_CHAT_WRAPPER}）"
    );
}

/// 边界：命令总数 +1（43 → 44），两份清单条目数持衡（漏登记或重复登记即红）。
#[test]
fn 登记面_命令总数加一且两清单持衡() {
    assert_eq!(
        COMMAND_NAMES.len(),
        45,
        "本变更恰 +1（44 → 45）后的命令总数"
    );
    assert_eq!(
        COMMAND_WRAPPERS.len(),
        45,
        "包装清单条目数与 invoke 清单持衡（一一对应）"
    );
}

/// 正向：生成绑定逐字（三参透传 + 返回 `SessionPage`；无 `id` 参即与 change chat
/// 版签名的区分点）。
#[test]
fn 登记面_生成绑定逐字三参透传() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert!(
        content.contains(ARCHI_CHAT_WRAPPER_LINE),
        "生成绑定形态不符（参数面 / 返回类型 / invoke 目标），期望行:\n{ARCHI_CHAT_WRAPPER_LINE}"
    );
    let section = type_section(&content, "SessionPage");
    assert_eq!(
        field_names(&section),
        vec!["sessions", "total"],
        "返回类型 `SessionPage` 两字段（分页信封），实际段:\n{section}"
    );
}

/// 异常：唯一登记——两份清单内计数恰 1，产物内包装与 invoke 目标各恰 1 次（无
/// 重复项）。
#[test]
fn 登记面_新命令唯一登记() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert_eq!(
        COMMAND_NAMES
            .iter()
            .filter(|name| **name == ARCHI_CHAT_COMMAND)
            .count(),
        1,
        "invoke 名在清单中恰登记一次"
    );
    assert_eq!(
        COMMAND_WRAPPERS
            .iter()
            .filter(|wrapper| **wrapper == ARCHI_CHAT_WRAPPER)
            .count(),
        1,
        "包装名在清单中恰登记一次"
    );
    assert_eq!(
        content.matches(&format!("{ARCHI_CHAT_WRAPPER}:")).count(),
        1,
        "包装在产物中恰一次（无重复登记）"
    );
    assert_eq!(
        content
            .matches(&format!("\"{ARCHI_CHAT_COMMAND}\""))
            .count(),
        1,
        "invoke 目标在产物中恰一次"
    );
}

/// 回归：既有条目零位移——既有 43 条逐条仍在册（含 `list_change_chat_sessions` /
/// `agent_sessions` / 五条 `archi_*`），`dto.ts` 星号 re-export 反向锚不变。
#[test]
fn 登记面_既有条目零位移() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for (name, wrapper) in [
        ("list_change_chat_sessions", "listChangeChatSessions"),
        ("agent_sessions", "agentSessions"),
        ("agent_start", "agentStart"),
        ("archi_graph", "archiGraph"),
        ("archi_module", "archiModule"),
        ("archi_read_doc", "archiReadDoc"),
        ("archi_change_summary", "archiChangeSummary"),
        ("archi_set_gate", "archiSetGate"),
    ] {
        assert!(
            COMMAND_NAMES.contains(&name),
            "既有 invoke 名仍在册：{name}"
        );
        assert!(
            COMMAND_WRAPPERS.contains(&wrapper),
            "既有包装名仍在册：{wrapper}"
        );
        assert!(
            content.contains(&format!("\"{name}\"")),
            "产物仍含既有 invoke 目标：{name}"
        );
    }
    // 新命令紧跟同族命令之后（既有条目零位移，非尾部改写）
    let chat_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "list_change_chat_sessions")
        .expect("同族命令仍在册");
    assert_eq!(
        COMMAND_NAMES[chat_index + 1],
        ARCHI_CHAT_COMMAND,
        "新命令紧跟 list_change_chat_sessions 之后登记"
    );

    let dto_shim = fs::read_to_string(dto_shim_path()).expect("读取 dto.ts 应成功");
    assert!(
        dto_shim.contains("export type * from './generated/bindings';"),
        "dto.ts 星号 re-export 反向锚不变"
    );
}

// ---------------------------------------------------------------------------
// `archi_drift` 命令面（archi-check-and-gate，AC-7 / AC-15 / AC-20）：唯一登记面
// 补一行 → 产物出线（命令 + `ArchiDriftMode` 联合 + 15 类报告 DTO + 步词新臂）
// ---------------------------------------------------------------------------

/// 架构漂移检查命令的 invoke 名 / 包装名（唯一登记面补录的唯一新命令）。
const ARCHI_DRIFT_COMMAND: &str = "archi_drift";
const ARCHI_DRIFT_WRAPPER: &str = "archiDrift";

/// 架构漂移报告出线类型名（14 结构 / 词汇段 + `ArchiDriftMode` 命令参类型 =
/// 16 条；`Report` 短名经 `pub use` 别名出线为 `ArchiDriftReport`）。
const ARCHI_DRIFT_DTO_TYPES: [&str; 16] = [
    "ArchiCheckStatus",
    "ArchiDriftEntry",
    "ArchiDriftKind",
    "ArchiDriftMode",
    "ArchiDriftModel",
    "ArchiDriftObservation",
    "ArchiDriftOccurrence",
    "ArchiDriftReport",
    "ArchiDriftSuppressedEntry",
    "ArchiExcludedFile",
    "ArchiGateCount",
    "ArchiMatchedElement",
    "ArchiObservationKind",
    "ArchiScanReport",
    "ArchiSuppressVia",
    "ArchiUnresolvedFile",
];

/// 正向（AC-20）：`all_commands!` 登记面 +1（尾部登记，登记序即
/// `generate_handler` 序）→ 两份补录清单 44 → 45、尾锚由 `archi_set_gate` 移到
/// `archi_drift`，且清单每条都能在产物命中（无遗漏）。
#[test]
fn archi_drift登记面加一且尾锚随动() {
    assert!(
        COMMAND_NAMES.contains(&ARCHI_DRIFT_COMMAND),
        "invoke 命令名补录（{ARCHI_DRIFT_COMMAND}）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&ARCHI_DRIFT_WRAPPER),
        "生成包装名补录（{ARCHI_DRIFT_WRAPPER}）"
    );
    assert_eq!(
        COMMAND_NAMES.last(),
        Some(&ARCHI_DRIFT_COMMAND),
        "尾部登记即尾锚（登记序 = generate_handler 序）"
    );
    assert_eq!(
        COMMAND_WRAPPERS.last(),
        Some(&ARCHI_DRIFT_WRAPPER),
        "包装清单同序尾锚"
    );
    // 前一条 archi 命令下标零位移（尾部追加不改既有条目）
    let gate_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "archi_set_gate")
        .expect("既有 archi 命令仍在册");
    assert_eq!(
        COMMAND_NAMES[gate_index + 1],
        ARCHI_DRIFT_COMMAND,
        "新命令紧跟 archi_set_gate 之后登记"
    );

    // 清单每条都能在产物命中（登记 → 产物单向覆盖）
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    for (name, wrapper) in COMMAND_NAMES.iter().zip(COMMAND_WRAPPERS.iter()) {
        assert!(
            content.contains(&format!("\"{name}\"")),
            "产物缺 invoke 命令名 {name}（登记了但产物缺席）"
        );
        assert!(
            content.contains(&format!("{wrapper}:")),
            "产物缺命令包装名 {wrapper}（登记了但产物缺席）"
        );
    }
}

/// 正向（AC-20）：产物出线——命令包装逐字（两参透传 + `ArchiDriftMode` 参类型 +
/// `ArchiDriftReport` 返回）、`ArchiDriftMode` 二值联合、15 类报告 DTO 在案、
/// 步词汇三枚举新臂在案。
#[test]
fn archi_drift产物出线与漂移报告dto在案() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 命令包装逐字（root + mode 两参透传，invoke 目标为 snake 命令名）
    let wrapper_line = format!(
        "{ARCHI_DRIFT_WRAPPER}: (root: string, mode: ArchiDriftMode) => __TAURI_INVOKE<ArchiDriftReport>(\"{ARCHI_DRIFT_COMMAND}\", {{ root, mode }}),"
    );
    assert!(
        content.contains(&wrapper_line),
        "生成绑定形态不符（参数面 / 返回类型 / invoke 目标），期望行:\n{wrapper_line}"
    );

    // `ArchiDriftMode` 二值联合（serde lowercase 线词）
    let mode_section = type_section(&content, "ArchiDriftMode");
    for variant in ["\"full\"", "\"incremental\""] {
        assert!(
            mode_section.contains(variant),
            "ArchiDriftMode 缺变体 {variant}，实际段:\n{mode_section}"
        );
    }

    // 15 类报告 DTO + 命令参类型：清单 ↔ 产物双向在案
    for dto in ARCHI_DRIFT_DTO_TYPES {
        assert!(DTO_TYPES.contains(&dto), "DTO_TYPES 清单应在册 {dto}");
        assert!(
            content.contains(&format!("export type {dto} =")),
            "产物缺出线 DTO 类型 {dto}"
        );
    }

    // 步词汇两枚举新臂（AC-16 全链路可见面；StepNode 为进程内相位表类型，
    // 不出 IPC，其词面由 workflow `state_test` 承载）
    assert!(
        type_section(&content, "ChangeStepKind").contains("\"archiCheck\""),
        "ChangeStepKind 缺 archiCheck 变体（活面 camel 词）"
    );
    assert!(
        type_section(&content, "RunStepKind").contains("\"archi_check\""),
        "RunStepKind 缺 archi_check 变体（落库 snake 词）"
    );
}

/// 边界：`ArchiDriftReport` 字段面出线（14 字段、camelCase 键、`status` 走
/// `ArchiCheckStatus`、`mode` 走 `ArchiDriftMode`、`change` / `base` 可空）。
#[test]
fn archi_drift报告字段面出线逐键() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let report = type_section(&content, "ArchiDriftReport");
    assert_eq!(
        field_names(&report),
        vec![
            "version",
            "timestamp",
            "mode",
            "root",
            "change",
            "base",
            "modelHash",
            "model",
            "scan",
            "errors",
            "warnings",
            "suppressed",
            "observations",
            "status",
        ],
        "ArchiDriftReport 十四字段按声明序出线（camelCase 键），实际段:\n{report}"
    );
    for literal in [
        "mode: ArchiDriftMode,",
        "change: string | null,",
        "base: string | null,",
        "modelHash: string,",
        "model: ArchiDriftModel,",
        "scan: ArchiScanReport,",
        "errors: ArchiDriftEntry[],",
        "warnings: ArchiDriftEntry[],",
        "suppressed: ArchiDriftSuppressedEntry[],",
        "observations: ArchiDriftObservation[],",
        "status: ArchiCheckStatus,",
    ] {
        assert!(
            report.contains(literal),
            "ArchiDriftReport 字段出线不符，期望含 {literal}，实际段:\n{report}"
        );
    }
    // 契约面：结论由门禁侧派生，产物不携带 `passed` 字段（注释文字不算线面）
    assert!(
        !field_names(&report).contains(&"passed".to_owned()),
        "报告线面 MUST NOT 携带 passed 字段（结论派生在门禁侧，不双写）"
    );
    assert!(!report.contains("passed:"), "线面无 passed 键");
}

/// 异常（双向核对）：产物命令包装集合与补录清单互为镜像——「产物有而清单无」
/// 与「清单有而产物无」双向失败（漏登记即红）。
#[test]
fn archi_drift清单与产物命令面互为镜像() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 产物面 → 清单面：逐行解析命令包装声明（`\t<wrapper>: (` 形态）
    let product_wrappers: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let (wrapper, rest) = trimmed.split_once(": (")?;
            if !rest.contains("__TAURI_INVOKE") {
                return None;
            }
            Some(wrapper.to_owned())
        })
        .collect();
    assert!(
        product_wrappers.contains(&ARCHI_DRIFT_WRAPPER.to_owned()),
        "产物命令面应含 {ARCHI_DRIFT_WRAPPER}"
    );
    // 范围限定在 archi 命令族（其余历史缺录为既有滞后债，不在本变更范围）
    let missing_in_list: Vec<&String> = product_wrappers
        .iter()
        .filter(|wrapper| wrapper.starts_with("archi"))
        .filter(|wrapper| !COMMAND_WRAPPERS.contains(&wrapper.as_str()))
        .collect();
    assert!(
        missing_in_list.is_empty(),
        "产物有而清单无（archi 族漏登记）：{missing_in_list:?}"
    );
    // 清单有而产物无：archi 命令族逐条回查（六条在册 + 一条会话清单同名族）
    for wrapper in [
        "archiGraph",
        "archiModule",
        "archiReadDoc",
        "archiChangeSummary",
        "archiSetGate",
        "archiDrift",
    ] {
        assert!(
            COMMAND_WRAPPERS.contains(&wrapper),
            "archi 族命令应在册（{wrapper}）"
        );
        assert!(
            product_wrappers.iter().any(|item| item == wrapper),
            "清单有而产物无（{wrapper}）"
        );
    }

    // 清单面 → 产物面：archi_drift 唯一登记（无重复项）
    assert_eq!(
        COMMAND_NAMES
            .iter()
            .filter(|name| **name == ARCHI_DRIFT_COMMAND)
            .count(),
        1,
        "invoke 名在清单中恰登记一次"
    );
    assert_eq!(
        content
            .matches(&format!("\"{ARCHI_DRIFT_COMMAND}\""))
            .count(),
        1,
        "invoke 目标在产物中恰一次（无重复登记）"
    );
}

/// 边界：幂等重导出——`export_bindings()` 连续两次产物字节相等（`bindings:check`
/// 零 diff 前提），手写旁路零接触。
#[test]
fn archi_drift幂等重导出字节零漂移() {
    let _lock = lock();
    let first = authoritative_snapshot();
    let second = authoritative_snapshot();
    assert_eq!(first, second, "连续两次重导出字节相等（零 diff）");
    let text = String::from_utf8(first).expect("产物为 UTF-8 文本");
    assert!(
        text.contains(&format!("{ARCHI_DRIFT_WRAPPER}:")),
        "重导出后仍含 archi_drift 包装（无生成面回归）"
    );
}

/// 边界（锁的互斥面）：同进程第二个句柄在持锁期间 `try_lock` 必得
/// `WouldBlock`——OS 文件锁的语义不依赖是否跨进程，故本用例无需并发调度即可
/// 证明互斥成立（此前 `create_new` 方案在 Windows 上会因 delete-pending 返回
/// `os error 5`，见 `lock()` 头注释）。
#[test]
fn 产物文件锁同进程第二句柄被拒() {
    let _lock = lock();
    let path = lock_file_path();
    let second = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("打开第二个句柄应成功（文件常驻，非 create_new）");
    assert!(
        matches!(second.try_lock(), Err(fs::TryLockError::WouldBlock)),
        "持锁期间第二句柄必须被拒（WouldBlock 而非 os error 5）"
    );
    // 锁文件常驻：不因守卫释放而消失（删除会重开 delete-pending 窗口）
    assert!(path.is_file(), "锁文件常驻（MUST NOT 在 Drop 中删除）");
}

/// 边界（释放语义）：守卫 `Drop` 后同一进程可立刻再次取锁，且锁文件仍在——
/// 这是与旧实现的分水岭：旧 `Drop` 删文件（Windows 上留下 delete-pending 窗口），
/// 新实现只关句柄（OS 自动释放锁）。
#[test]
fn 产物文件锁drop后立即可重取且文件常驻() {
    {
        let _guard = lock();
    }
    let path = lock_file_path();
    assert!(path.is_file(), "Drop 后锁文件常驻（MUST NOT 删除）");
    // 立即重取：锁已随句柄关闭释放，无需等待轮询
    let _again = lock();
}
