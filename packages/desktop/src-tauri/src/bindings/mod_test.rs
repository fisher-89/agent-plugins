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
    "execOutputPath",
    "readExecCommand",
    "readExecOutputDelta",
    "archiveChange",
    "deleteChange",
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
    "listModelCatalog",
    "agentStart",
    "agentStop",
    "agentAnswer",
    "agentSteer",
    "agentSessions",
    "agentSessionTranscript",
    "sessionDetail",
    "listChangeChatSessions",
    "listArchiChatSessions",
    "listConfigChatSessions",
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
    "workspaceConfigApply",
    "appEnvironment",
    "archiGraph",
    "archiModule",
    "archiChangeSummary",
    "archiSetGate",
    "archiDrift",
    "archiFileTree",
    "archiApiCheck",
    "archiApiGenerate",
    "archiRelation",
    "readFile",
    "notificationSubscribe",
    "notificationList",
    "notificationMarkRead",
    "notificationMarkAllRead",
    "notificationRecord",
];

/// 38 条命令的 IPC 命令名（snake_case，invoke 目标；补录口径同上）。
const COMMAND_NAMES: &[&str] = &[
    "list_changes",
    "get_change_detail",
    "read_artifact",
    "exec_output_path",
    "read_exec_command",
    "read_exec_output_delta",
    "archive_change",
    "delete_change",
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
    "list_model_catalog",
    "agent_start",
    "agent_stop",
    "agent_answer",
    "agent_steer",
    "agent_sessions",
    "agent_session_transcript",
    "session_detail",
    "list_change_chat_sessions",
    "list_archi_chat_sessions",
    "list_config_chat_sessions",
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
    "workspace_config_apply",
    "app_environment",
    "archi_graph",
    "archi_module",
    "archi_change_summary",
    "archi_set_gate",
    "archi_drift",
    "archi_file_tree",
    "archi_api_check",
    "archi_api_generate",
    "archi_relation",
    "read_file",
    "notification_subscribe",
    "notification_list",
    "notification_mark_read",
    "notification_mark_all_read",
    "notification_record",
];

/// 全部出线 DTO 类型名（产物 Types 段的 `export type` 全集）。
const DTO_TYPES: &[&str] = &[
    "ActivePhase",
    "ExecOutputChunk",
    "AgentBlock",
    "AgentEffortTiers",
    "AgentEngineKind",
    "AgentEvent",
    "AgentEventKind",
    "AgentInstanceRecord",
    "AgentModelTiers",
    "AgentPermissionMode",
    "AgentProviderRecord",
    "AgentProviderTuning",
    "AgentRunMessage",
    "AgentRunStatus",
    "SessionPage",
    "SessionPreambleInfo",
    "SessionRow",
    "SessionStats",
    "SessionSummary",
    "TurnSummary",
    "ArchiveGroup",
    "ArchiveOutcome",
    "CreateOutcome",
    "DeleteOutcome",
    "ArtifactDescriptor",
    "ArtifactEnvelope",
    "AttemptRecord",
    "CatalogModel",
    "CatalogProtocol",
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
    "ExploreRecord",
    "ExploreScanEntry",
    "FileNode",
    "FileSlice",
    "FileStat",
    "FileWatchEvent",
    "LanguageStats",
    "ModelCatalog",
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
    "ArchiApiCheck",
    "ArchiApiMeta",
    "ArchiApiWriteOutcome",
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
    "ArchiFileEntry",
    "ArchiFileTree",
    "ArchiGateChange",
    "ArchiGateCount",
    "ArchiGraph",
    "ArchiMatchedElement",
    "ArchiModuleDetail",
    "ArchiObservationKind",
    "ArchiRelation",
    "ArchiRelationDetail",
    "ArchiScanReport",
    "ArchiSuppressVia",
    "ArchiUnresolvedFile",
    "EdgeMark",
    "ElementKind",
    "FileEntryKind",
    "FileTreeMark",
    "GateTier",
    "NodeMark",
    // 通知域（desktop-notification-hub）出线类型
    "Notification",
    "NotificationFeedbackInput",
    "NotificationKind",
    "NotificationLevel",
    "NotificationPage",
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

/// zod 产物路径（与 `export_bindings` 内 `src/types/generated` 定位同式）。
fn generated_zod_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR 应有父目录")
        .join("src/types/generated/bindings.zod.ts")
}

/// zod 权威产物文本：重导出（幂等确定性）后读取（同一 builder 同步产出
/// `bindings.ts` 与 `bindings.zod.ts`，调用方须持产物文件锁）。
fn authoritative_zod_snapshot() -> String {
    export_bindings().expect("export_bindings 应成功");
    fs::read_to_string(generated_zod_path()).expect("读取 zod 产物文件应成功")
}

/// zod 产物中某 `export const <name> =` 声明的完整文本段（至下一个顶层
/// `export` 前）。
fn zod_section(content: &str, name: &str) -> String {
    let marker = format!("export const {name} = ");
    let start = content
        .find(&marker)
        .unwrap_or_else(|| panic!("zod 产物缺声明 {name}"));
    let body_start = start + marker.len();
    let end = content[body_start..]
        .find("\nexport ")
        .map_or(content.len(), |offset| body_start + offset);
    content[start..end].to_owned()
}

/// 内存副本：删除含 `needle` 的行（首处），产物文件本体零改动。
fn without_line(text: &str, needle: &str) -> String {
    text.lines()
        .filter(|line| !line.contains(needle))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 期待断言体 panic 并取回 panic 文案（文案本身是判据的一部分：缺键指认）。
fn panic_text(body: impl FnOnce()) -> String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(()) => panic!("期望断言失败 panic，实际通过"),
        Err(payload) => match payload.downcast_ref::<String>() {
            Some(text) => text.clone(),
            None => match payload.downcast_ref::<&'static str>() {
                Some(text) => (*text).to_string(),
                None => panic!("panic 载荷既非 String 也非 &str，无法核对文案"),
            },
        },
    }
}

/// `ProjectConfig` 出线形态断言（入参化文本：正向喂权威产物段，异常喂内存
/// 副本）；失败文案指认缺失键，并按声明序校验三字段先后。
fn assert_project_config_section(section: &str) {
    let fields = [
        "root: string,",
        "language: string | null,",
        "dependsOn: string[] | null,",
    ];
    let mut last = 0;
    for field in fields {
        let position = section
            .find(field)
            .unwrap_or_else(|| panic!("ProjectConfig 缺字段出线 {field}，实际 {section}"));
        assert!(
            position >= last,
            "ProjectConfig 字段出线序漂移（{field} 应位于前序字段之后），实际 {section}"
        );
        last = position;
    }
}

/// `ProjectConfigSchema` 出线形态断言（入参化文本：正向喂权威 zod 产物段，
/// 异常喂内存副本）；失败文案指认缺失 getter。
fn assert_project_config_schema(section: &str) {
    assert!(
        section
            .trim_start()
            .starts_with("export const ProjectConfigSchema = z.strictObject({"),
        "ProjectConfigSchema 应为 strictObject 包裹（两 getter 落同一对象内），实际 {section}"
    );
    for getter in [
        "get root() { return z.string(); },",
        "get language() { return z.string().nullable(); },",
        "get dependsOn() { return z.array(z.string()).nullable(); },",
    ] {
        assert!(
            section.contains(getter),
            "ProjectConfigSchema 缺 {getter}，实际 {section}"
        );
    }
    assert!(
        section.trim_end().ends_with("});"),
        "ProjectConfigSchema 声明应以 `}});` 收口，实际 {section}"
    );
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
// 节一致）：`SessionInjections` / `FinishSpec` / `TurnQuestion` 是引擎侧会话
// 注入词汇（`crates/core/agent/src/runner.rs`，serde 线形由 engine crate 自持），
// **不属 IPC 面**——`all_commands!` / `collect_commands` 的出线参数与返回类型里
// 没有它，产物 `src/types/generated/bindings.ts` 全文零该类型名（specta 不出线
// = 预期形态，非缺漏）。
//
// 故本节不设「变体出线」正向断言（不虚构产物面），以后述反向锚定固定口径：
// 引擎侧词汇一旦被误接到 IPC 出线面即红；同源语义（4 组合 round-trip / 旧线
// 兼容 / 非法线值拒绝）由 `crates/core/agent/src/runner_test.rs` 的
// `FinishSpec` serde 用例承载。功能代码零改动。
// ---------------------------------------------------------------------------

#[test]
fn 引擎侧会话注入词汇不出线ipc产物() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for engine_side in ["SessionInjections", "FinishSpec", "TurnQuestion"] {
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

    // ProjectConfig：`root` 锚点 + 两可选协议字段（`Option` 出线 `null`、camelCase）
    let project = type_section(&content, "ProjectConfig");
    assert_project_config_section(&project);
    assert!(
        config.contains("projects: ProjectConfig[],"),
        "WorkspaceConfig.projects 容器形态不变（元素形状变宽）"
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

#[test]
fn 项目配置出线缺两新键的副本被抓且文案指认缺键() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let section = type_section(&content, "ProjectConfig");

    // 副本一：删除 language 行
    let without_language = without_line(&section, "language: string | null,");
    let text = panic_text(|| assert_project_config_section(&without_language));
    assert!(
        text.contains("language: string | null,"),
        "删 language 行的副本应被断言抓且指认缺键，实际文案 {text:?}"
    );

    // 副本二：删除 dependsOn 行
    let without_depends_on = without_line(&section, "dependsOn: string[] | null,");
    let text = panic_text(|| assert_project_config_section(&without_depends_on));
    assert!(
        text.contains("dependsOn: string[] | null,"),
        "删 dependsOn 行的副本应被断言抓且指认缺键，实际文案 {text:?}"
    );
}

#[test]
fn 项目配置zod_schema含两新键getter且strict_object包裹() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();

    let schema = zod_section(&content, "ProjectConfigSchema");
    assert_project_config_schema(&schema);

    // 容器形态不变：projects 仍为 ProjectConfigSchema 数组
    let workspace = zod_section(&content, "WorkspaceConfigSchema");
    assert!(
        workspace.contains("get projects() { return z.array(ProjectConfigSchema); },"),
        "WorkspaceConfigSchema.projects 应仍为 z.array(ProjectConfigSchema)，实际 {workspace}"
    );
}

#[test]
fn 项目配置zod_schema缺depends_on的副本被抓且文案指认缺键() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();
    let schema = zod_section(&content, "ProjectConfigSchema");

    let without_depends_on = without_line(&schema, "get dependsOn()");
    let text = panic_text(|| assert_project_config_schema(&without_depends_on));
    assert!(
        text.contains("get dependsOn()"),
        "删 dependsOn getter 的副本应被断言抓且指认缺键，实际文案 {text:?}"
    );
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
    // 合并侧两变更各补录一条（架构页文件树 + agent_answer）：条目数 48 → 50；
    // 其后补 agent_steer（50 → 51）；archi_relation（56 → 57）；本变更再补五条
    // 通知域命令（57 → 62，登记序即 all_commands! 末尾）
    assert_eq!(
        COMMAND_NAMES.len(),
        62,
        "命令清单本变更补录五条（通知域，57 → 62）"
    );
    assert_eq!(
        COMMAND_WRAPPERS.len(),
        62,
        "包装清单本变更补录五条（通知域，57 → 62）"
    );
    assert!(
        COMMAND_NAMES.contains(&"agent_steer"),
        "invoke 名补录（agent_steer）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&"agentSteer"),
        "包装名补录（agentSteer）"
    );
    assert_eq!(COMMAND_NAMES.first(), Some(&"list_changes"));
    assert_eq!(
        COMMAND_NAMES.last(),
        Some(&"notification_record"),
        "尾锚随通知域五命令后移（登记序末尾）"
    );
    assert_eq!(
        COMMAND_WRAPPERS.last(),
        Some(&"notificationRecord"),
        "包装尾锚同式随动"
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
    // 本变更新增命令（ask 应答，通用侧）
    assert!(
        COMMAND_NAMES.contains(&"agent_answer"),
        "invoke 名补录（agent_answer）"
    );
    assert!(
        COMMAND_WRAPPERS.contains(&"agentAnswer"),
        "包装名补录（agentAnswer）"
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
        111,
        "清单 110 → 111：本变更补录一类型（SessionPreambleInfo）"
    );
    assert_eq!(DTO_TYPES.first(), Some(&"ActivePhase"), "首锚持衡");
    assert_eq!(
        DTO_TYPES.last(),
        Some(&"NotificationPage"),
        "尾锚随通知域五出线类型追加"
    );
    assert!(
        DTO_TYPES.contains(&"NodeMark"),
        "封闭词汇末项条目仍在册（不因追加丢失）"
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
// 归档链退役
// ---------------------------------------------------------------------------

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
// app_environment 登记面
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

/// 登记面异常面：重复导出逐字节一致（入库物一致性的单测等价面——缺登记 /
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

    // 清单末条随尾部追加随动：通知域五条登记在 `readFile` 之后，
    // `notificationRecord` 因而是末条（既有条目零位移——`appEnvironment` /
    // `archiDrift` / `readFile` 仍在册且各恰一次，见上）
    let index = content.find("notificationRecord:").expect("包装在场");
    let tail = &content[index..];
    let line_end = tail.find('\n').expect("包装行有换行");
    assert!(
        tail[line_end + 1..].starts_with("};"),
        "notificationRecord 为清单末条（生成物中其后即对象收尾）"
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
// archi 章节：命令族与架构域出线类型在册（AC-20 的守卫面）
// ---------------------------------------------------------------------------

/// 架构域命令族（invoke 名 / 包装名成对）。
const ARCHI_COMMANDS: [(&str, &str); 9] = [
    ("archi_graph", "archiGraph"),
    ("archi_module", "archiModule"),
    ("archi_change_summary", "archiChangeSummary"),
    ("archi_set_gate", "archiSetGate"),
    ("archi_drift", "archiDrift"),
    ("archi_file_tree", "archiFileTree"),
    ("archi_api_check", "archiApiCheck"),
    ("archi_api_generate", "archiApiGenerate"),
    ("archi_relation", "archiRelation"),
];

/// 架构域出线类型名（`Archi*` 投影 + 五组封闭词汇）。
const ARCHI_DTO_TYPES: [&str; 22] = [
    "ArchiApiCheck",
    "ArchiApiMeta",
    "ArchiApiWriteOutcome",
    "ArchiChangeSummary",
    "ArchiDependedBy",
    "ArchiDependsOn",
    "ArchiFileEntry",
    "ArchiFileTree",
    "ArchiDiagnostic",
    "ArchiDiagnosticKind",
    "ArchiElement",
    "ArchiGateChange",
    "ArchiGraph",
    "ArchiModuleDetail",
    "ArchiRelation",
    "ArchiRelationDetail",
    "EdgeMark",
    "ElementKind",
    "FileEntryKind",
    "FileTreeMark",
    "GateTier",
    "NodeMark",
];

/// 正向：命令族在册（清单 + 产物双面）；既有条目零位移（尾部追加）。
#[test]
fn archi命令族在册且既有条目零位移() {
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

    // 尾部追加：新命令位于 `app_environment` 之后且序固定（通知域五条在本族
    // 之后按登记序追加，故尾部清单含 16 条）
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
            "archi_change_summary",
            "archi_set_gate",
            "archi_drift",
            "archi_file_tree",
            "archi_api_check",
            "archi_api_generate",
            "archi_relation",
            "read_file",
            "notification_subscribe",
            "notification_list",
            "notification_mark_read",
            "notification_mark_all_read",
            "notification_record",
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

    // `ArchiGraph` 三数组形状（停摆判据 = diagnostics 非空）
    let graph = type_section(&content, "ArchiGraph");
    assert_eq!(
        field_names(&graph),
        vec!["elements", "relations", "diagnostics"],
        "ArchiGraph 三字段按声明序出线（元素 / 关系 / 诊断），实际段:\n{graph}"
    );

    // 退役类型零声明（旧关联面的三个类型 + 旧警告两类型）
    let retired = [
        format!("{}{}", "Doc", "Ref"),
        format!("{}{}", "Doc", "Kind"),
        format!("{}{}", "Doc", "Source"),
        format!("{}{}", "Warning", "Kind"),
        "ArchiWarning".to_owned(),
    ];
    for name in &retired {
        assert!(
            !content.contains(&format!("export type {name} =")),
            "退役类型零出线声明：{name}"
        );
    }

    // `ArchiModuleDetail` 的 `doc` 单值槽位
    let detail = type_section(&content, "ArchiModuleDetail");
    assert_eq!(
        field_names(&detail),
        vec![
            "element",
            "dependsOn",
            "dependedBy",
            "children",
            "doc",
            "api",
            "resolvedEntry"
        ],
        "ArchiModuleDetail 字段面按声明序出线（`doc` 单值 + `resolvedEntry` 末位），实际段:\n{detail}"
    );
    // 薄条目不出线：`hasPath` 只服务 agent 工具（IPC 面无消费方 ⇒ 零声明）
    assert!(
        !content.contains("export type ArchiElementBrief ="),
        "`ArchiElementBrief` 零出线（AC-10：产物增面恰为 resolvedEntry 三件）"
    );

    // 入口解析结果：三字段皆不省略（`via` 封闭二值）
    let resolved = type_section(&content, "ArchiResolvedEntry");
    assert_eq!(
        field_names(&resolved),
        vec!["path", "via", "problem"],
        "ArchiResolvedEntry 三字段按声明序出线，实际段:
{resolved}"
    );
    assert!(
        content.contains("export type EntryVia ="),
        "`EntryVia` 二值封闭类型出线"
    );

    let element = type_section(&content, "ArchiElement");
    let element_fields = field_names(&element);
    let entry_at = element_fields
        .iter()
        .position(|field| field == "entry")
        .expect("entry 在字段面");
    assert_eq!(
        element_fields.get(entry_at + 1).map(String::as_str),
        Some("doc"),
        "`doc` 紧跟 `entry`：{element_fields:?}"
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
    // 降级哨兵只落 6 个 Channel 形参位（agentStart / agentSessionWatch /
    // changeFlowStart / changeFlowWatch / watchSubscribe / notificationSubscribe
    // 的 onEvent）
    assert_eq!(
        zod.matches("degraded(").count(),
        6,
        "degraded( 恰 6 处（Channel 形参位）"
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

/// 边界：命令总数 51 → 62（累计补录含本变更通知域五条），两份清单条目数持衡
///（漏登记或重复登记即红）。
#[test]
fn 登记面_命令总数加三且两清单持衡() {
    assert_eq!(COMMAND_NAMES.len(), 62, "累计补录后的命令总数（51 → 62）");
    assert_eq!(
        COMMAND_WRAPPERS.len(),
        62,
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

/// 正向（AC-20）：`archi_drift` 在册（`read_file` 之后为通用读命令，本变更再追加
/// 通知域五条，尾锚随动）。
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
        Some(&"notification_record"),
        "尾锚为通知域落库命令（通知域五条最后登记）"
    );
    assert_eq!(
        COMMAND_WRAPPERS.last(),
        Some(&"notificationRecord"),
        "包装清单同序尾锚"
    );
    // 既有前缀零位移：前 57 条的尾锚仍是通用读命令
    assert_eq!(
        COMMAND_NAMES[..57].last(),
        Some(&"read_file"),
        "既有 57 条前缀零位移（read_file 仍为前缀末条）"
    );
    // 前一条 archi 命令下标零位移（尾部追加不改既有条目）
    let gate_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "archi_set_gate")
        .expect("既有 archi 命令仍在册");
    assert_eq!(
        COMMAND_NAMES[gate_index + 1],
        ARCHI_DRIFT_COMMAND,
        "archi_drift 紧跟 archi_set_gate 之后登记"
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
            "apiFindings",
            "status",
        ],
        "ArchiDriftReport 十四字段 + apiFindings（末位追加）按声明序出线（camelCase 键），实际段:\n{report}"
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
        "apiFindings: ArchiApiFinding[],",
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
    // 清单有而产物无：archi 命令族逐条回查（命令族在册 + 架构指纹命令）
    for wrapper in [
        "archiGraph",
        "archiModule",
        "archiChangeSummary",
        "archiSetGate",
        "archiDrift",
        "archiFileTree",
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

// ---------------------------------------------------------------------------
// 三条 exec 记录读命令的登记面（change-detail-live-stdout，AC-14）
//
// 被测面：`all_commands!` 在 `read_artifact` 之后的三条 exec 记录读命令——两份
// 补录清单（`COMMAND_NAMES` / `COMMAND_WRAPPERS`）逐名在册、总数锚 49、尾锚零
// 位移（`read_file` 是末条）、生成包装逐字形态、唯一登记（各恰一次）、
// `ExecOutputChunk` 出线类型在册。
//
// Mock 策略（见 test-design 本节 Mock策略表）：生成物真实（`authoritative_snapshot()`
// 读盘快照 + 既有跨进程文件锁 `lock()` 串行化）；`all_commands!` 为真实宏；
// 本节零 IPC / 零 spawn。
// ---------------------------------------------------------------------------

/// 三条 exec 记录读命令的 invoke 名（登记序：紧跟 `read_artifact`）。
const NEW_COMMAND_NAMES: [&str; 3] = [
    "exec_output_path",
    "read_exec_command",
    "read_exec_output_delta",
];

/// 三条新命令的生成包装名（同序）。
const NEW_COMMAND_WRAPPERS: [&str; 3] =
    ["execOutputPath", "readExecCommand", "readExecOutputDelta"];

/// 三条包装的生成行逐字锚（参数键 lowerCamelCase / 返回类型 nullable 面）。
const NEWLINE: char = '\n';
const NEW_WRAPPER_LINES: [&str; 3] = [
    "execOutputPath: (root: string, changeName: string, execId: string) => __TAURI_INVOKE<string | null>(\"exec_output_path\", { root, changeName, execId }),",
    "readExecCommand: (root: string, changeName: string, execId: string) => __TAURI_INVOKE<string | null>(\"read_exec_command\", { root, changeName, execId }),",
    "readExecOutputDelta: (root: string, changeName: string, execId: string, fromOffset: number) => __TAURI_INVOKE<ExecOutputChunk | null>(\"read_exec_output_delta\", { root, changeName, execId, fromOffset }),",
];

/// 正向（参数化 3 命令）：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 各含三个新名字，
/// 且生成物逐条命中（invoke 名 + 包装行）。
#[test]
fn 登记面_三新命令在两清单与生成物各在册() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for (index, name) in NEW_COMMAND_NAMES.iter().enumerate() {
        let wrapper = NEW_COMMAND_WRAPPERS[index];
        assert!(COMMAND_NAMES.contains(name), "invoke 命令名补录（{name}）");
        assert!(
            COMMAND_WRAPPERS.contains(&wrapper),
            "生成包装名补录（{wrapper}）"
        );
        assert!(
            content.contains(&format!("\"{name}\"")),
            "生成物缺 invoke 命令名 {name}"
        );
        assert!(
            content.contains(NEW_WRAPPER_LINES[index]),
            "生成包装行形态不符（{wrapper}），期望:\n{}",
            NEW_WRAPPER_LINES[index]
        );
    }
}

/// 正向：总数锚 51 → 62（逐变更叠加，两份清单持衡、一一对应）；
/// 尾锚随通知域五条追加落到 `notification_record`，既有 57 条前缀零位移。
#[test]
fn 登记面_总数五十且尾锚零位移() {
    assert_eq!(COMMAND_NAMES.len(), 62, "命令清单总数 51 → 62");
    assert_eq!(
        COMMAND_WRAPPERS.len(),
        62,
        "包装清单条目数与 invoke 清单持衡（一一对应）"
    );
    assert_eq!(
        COMMAND_NAMES.last(),
        Some(&"notification_record"),
        "尾锚为通知域落库命令（通知域五条最后登记）"
    );
    assert_eq!(
        COMMAND_WRAPPERS.last(),
        Some(&"notificationRecord"),
        "包装清单同序尾锚"
    );
    assert_eq!(
        COMMAND_NAMES[..57].last(),
        Some(&"read_file"),
        "既有 57 条前缀零位移（尾部 read_file 仍是前缀末条）"
    );
    assert_eq!(
        COMMAND_WRAPPERS[..57].last(),
        Some(&"readFile"),
        "包装清单前缀同序尾锚"
    );
}

/// 异常（参数化 3 命令）：唯一登记——三命令在两清单与生成物中各恰 1 次
///（无重复登记）；已退役的 `read_exec_output` 不再出现在任一登记面或产物中。
#[test]
fn 登记面_唯一登记且退役命令零残留() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for (index, name) in NEW_COMMAND_NAMES.iter().enumerate() {
        let wrapper = NEW_COMMAND_WRAPPERS[index];
        assert_eq!(
            COMMAND_NAMES.iter().filter(|entry| *entry == name).count(),
            1,
            "{name} 在 invoke 清单恰 1 次"
        );
        assert_eq!(
            COMMAND_WRAPPERS
                .iter()
                .filter(|entry| *entry == &wrapper)
                .count(),
            1,
            "{wrapper} 在包装清单恰 1 次"
        );
        assert_eq!(
            content.matches(&format!("\"{name}\"")).count(),
            1,
            "生成物中 {name} 恰 1 次（无重复登记）"
        );
        assert_eq!(
            content.matches(&format!("{wrapper}:")).count(),
            1,
            "生成物中 {wrapper} 恰 1 次（无重复登记）"
        );
    }

    // 退役命令零残留：`read_exec_output` / `readExecOutput` 已退出登记面与产物
    assert!(
        !COMMAND_NAMES.contains(&"read_exec_output"),
        "read_exec_output 已退出 invoke 清单"
    );
    assert!(
        !COMMAND_WRAPPERS.contains(&"readExecOutput"),
        "readExecOutput 已退出包装清单"
    );
    assert!(
        !content.contains("\"read_exec_output\""),
        "产物不再含 read_exec_output invoke 目标"
    );
    assert!(
        !content.contains("readExecOutput: ("),
        "产物不再含 readExecOutput 包装行"
    );
}

/// 边界：登记序——生成物内 `execOutputPath` → `readExecCommand` →
/// `readExecOutputDelta` 三者相对序与 `all_commands!` 一致，且紧随
/// `read_artifact`（changes 命令组内）。
#[test]
fn 登记面_三条包装同序且紧随read_artifact() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let anchor = content
        .find("readArtifact: (")
        .expect("既有 readArtifact 包装在场");
    let tail = &content[anchor..];
    let offsets: Vec<usize> = NEW_COMMAND_WRAPPERS
        .iter()
        .map(|wrapper| {
            tail.find(&format!("{wrapper}: "))
                .unwrap_or_else(|| panic!("生成物缺 {wrapper} 包装"))
        })
        .collect();
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "三包装生成序 = all_commands! 登记序: {offsets:?}"
    );
    // 首条紧跟 readArtifact 行：其间只夹该命令的 JSDoc 注释，无其它命令包装
    let after_anchor_line = tail.find(NEWLINE).expect("readArtifact 行有换行");
    let between = &tail[after_anchor_line..offsets[0]];
    assert!(
        !between.contains("__TAURI_INVOKE<"),
        "其间无其它命令包装夹在中间: {between:?}"
    );
    // `all_commands!` 侧同序：三命令连续且同序，前驱为 `read_artifact`
    let positions: Vec<usize> = NEW_COMMAND_NAMES
        .iter()
        .map(|name| {
            COMMAND_NAMES
                .iter()
                .position(|entry| entry == name)
                .unwrap_or_else(|| panic!("{name} 在册"))
        })
        .collect();
    assert!(
        positions[1] == positions[0] + 1 && positions[2] == positions[0] + 2,
        "invoke 清单内三命令连续且同序: {positions:?}"
    );
    let read_artifact = COMMAND_NAMES
        .iter()
        .position(|name| *name == "read_artifact")
        .expect("read_artifact 在册");
    assert_eq!(
        positions[0],
        read_artifact + 1,
        "三命令紧跟 read_artifact 之后（changes 命令组内）"
    );
}

/// 边界：`ExecOutputChunk` 出线类型在册（`DTO_TYPES` 与生成物声明两面），字段
/// 恰 `nextOffset` / `text`（camelCase，`u64` → `number` 的线面）。
#[test]
fn 登记面_exec_output_chunk类型在册且两字段() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert!(
        DTO_TYPES.contains(&"ExecOutputChunk"),
        "DTO 清单补录 ExecOutputChunk"
    );
    assert!(
        content.contains("export type ExecOutputChunk ="),
        "生成物缺 ExecOutputChunk 类型声明"
    );
    let section = type_section(&content, "ExecOutputChunk");
    assert_eq!(
        field_names(&section),
        vec!["text", "nextOffset"],
        "ExecOutputChunk 两字段（camelCase 线面），实际段:\n{section}"
    );
    assert!(
        section.contains("nextOffset: number,"),
        "nextOffset 出线为 number（u64 经 bigint 配置转 number）"
    );
    assert!(section.contains("text: string,"), "text 出线为 string");
}

/// 正向（AC-14 组合行）：生成物 `commands` 表的三条新包装与 invoke 名严格
/// 一一对应——无「清单里有、生成物里没有」的幽灵条目（既有的「清单是产物子集」
/// 断言在本节以三命令定向复核）。
#[test]
fn 登记面_三命令清单与产物一一对应() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for (index, name) in NEW_COMMAND_NAMES.iter().enumerate() {
        let wrapper = NEW_COMMAND_WRAPPERS[index];
        let wrapper_pos = content
            .find(&format!("{wrapper}:"))
            .unwrap_or_else(|| panic!("产物缺包装 {wrapper}"));
        let invoke_pos = content
            .find(&format!("\"{name}\""))
            .unwrap_or_else(|| panic!("产物缺 invoke 名 {name}"));
        assert!(
            wrapper_pos < invoke_pos,
            "{wrapper} 的行内 invoke 目标为 {name}（包装行在前）"
        );
    }
}

// ---------------------------------------------------------------------------
// 事件信封新增可选键出线（agent-event-turn-id）：`bindings.ts` 的
// `turnId?: number | null` 摊平在 envelope 段内（D2 字段序 + D7 出线形态），
// `bindings.zod.ts` 的基座判型 + 六变体放行由既有 `allowed_object_keys` 机制
// 自动补齐。产物 MUST NOT 手改——本节全部断言只读权威快照（重导出后读盘）。
// ---------------------------------------------------------------------------

/// `AgentEvent` 摊平段内的轮归属字段行（可空可选形态，字面量单点）。
const TURN_ID_TS_FIELD: &str = "turnId?: number | null,";

/// zod 基座段的判型行（`z.number().nullable().optional()`）。
const TURN_ID_ZOD_BASE: &str = "get turnId() { return z.number().nullable().optional(); },";

/// zod 六变体逐一的放行行（与 `seq` / `timestampMs` 同式）。
const TURN_ID_ZOD_VARIANT: &str = "turnId: z.unknown().optional(),";

/// `AgentEvent` 的 zod 基座段（`z.strictObject({` 之后、`.and(z.union([` 之前）。
fn zod_agent_event_base_section(zod: &str) -> &str {
    let start = zod
        .find("export const AgentEventSchema = z.strictObject({")
        .expect("zod 产物缺 AgentEventSchema 基座声明")
        + "export const AgentEventSchema = z.strictObject({".len();
    let end = zod[start..]
        .find("}).and(z.union([")
        .map_or(zod.len(), |offset| start + offset);
    &zod[start..end]
}

/// `AgentEvent` 的 zod 变体段（`.and(z.union([` 之后至该声明收尾）。
fn zod_agent_event_variant_section(zod: &str) -> &str {
    let start = zod
        .find("export const AgentEventSchema = z.strictObject({")
        .expect("zod 产物缺 AgentEventSchema 声明");
    let end = zod[start..]
        .find("export const AgentEventKindSchema")
        .map_or(zod.len(), |offset| start + offset);
    &zod[start..end]
}

#[test]
fn agent_event出线含可选turn_id且字段序先于seq() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let section = type_section(&content, "AgentEvent");
    assert!(
        section.contains(TURN_ID_TS_FIELD),
        "AgentEvent 摊平段缺可选轮归属字段 {TURN_ID_TS_FIELD}，实际段:\n{section}"
    );
    // 字段声明序由 Rust 结构体字段序确定性派生（非 serde_json 运行期键序）
    let turn_pos = section.find(TURN_ID_TS_FIELD).expect("轮归属字段行在场");
    let seq_pos = section.find("seq: number,").expect("seq 字段行在场");
    let ts_pos = section
        .find("timestampMs: number,")
        .expect("timestampMs 字段行在场");
    assert!(
        turn_pos < seq_pos && seq_pos < ts_pos,
        "轮归属声明排在 seq / timestampMs 之前（D2 字段序），实际位置 {turn_pos} / {seq_pos} / {ts_pos}"
    );
}

#[test]
fn agent_event出线无相位伴生类型与必填或改名形态() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let section = type_section(&content, "AgentEvent");

    for forbidden in ["_Deserialize", "_Serialize"] {
        assert!(
            !content.contains(forbidden),
            "产物全文不得含相位分裂伴生名 {forbidden}（skip_serializing_if 被加上的机械判据）"
        );
    }
    assert!(
        !section.contains("turnId: number,"),
        "轮归属只允许可选可空形态（必填形态即契约破），实际段:\n{section}"
    );
    assert!(
        !content.contains("eventTurnId"),
        "轮归属不得改名（D9-a 明令禁止 eventTurnId）"
    );
    // 文档注释里出现 `turn_id`（Rust 字段名原文）是预期；判据是**字段声明行**
    // 不得为 snake_case
    let snake_case_field_lines: Vec<&str> = content
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("turn_id"))
        .collect();
    assert!(
        snake_case_field_lines.is_empty(),
        "产物字段声明行不得漏出 snake_case 键（出线为 camelCase），实际: {snake_case_field_lines:?}"
    );
}

#[test]
fn 轮行面与信封面同名不同型并存不冲突() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let event_section = type_section(&content, "AgentEvent");
    let turn_section = type_section(&content, "TurnSummary");
    assert!(
        event_section.contains(TURN_ID_TS_FIELD),
        "信封面 turnId 为可选可空"
    );
    assert!(
        turn_section.contains("turnId: number,"),
        "轮行面 turnId 为必填数字（既有），实际段:\n{turn_section}"
    );
    assert!(
        !event_section.contains("turnId: number,"),
        "信封面不得退化为必填形态（同名不同型的第一半）"
    );
    assert!(
        !turn_section.contains(TURN_ID_TS_FIELD),
        "轮行面不得退化为可选可空形态（同名不同型的第二半），实际段:
{turn_section}"
    );
}

#[test]
fn 命令面相依类型零漂移_信封多一键不改签名() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert!(
        content.contains("export type AgentRunMessage ="),
        "AgentRunMessage 出线在场"
    );
    assert!(
        content.contains("{ ipc: \"event\"; event: AgentEvent }"),
        "实时事件臂逐字在场"
    );
    assert!(
        content.contains("{ ipc: \"record\"; record: TurnSummary }"),
        "终态轮行臂逐字在场"
    );
    assert!(
        content.contains("agentSessionTranscript: (root: string, sessionId: string) => __TAURI_INVOKE<AgentEvent[]>(\"agent_session_transcript\", { root, sessionId })"),
        "转录命令签名零漂移（返回 AgentEvent 数组，命令面不改）"
    );
}

#[test]
fn 重导出后turn_id行零diff且幂等() {
    let _lock = lock();
    let _restore = RestoreOnDrop;

    let first = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let second = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert_eq!(
        first, second,
        "连续两次重导出逐字节零 diff（新键不引入不确定性输出）"
    );
    for content in [&first, &second] {
        assert_eq!(
            content.matches(TURN_ID_TS_FIELD).count(),
            1,
            "轮归属字段行在产物中恰一次（无重复落键 / 无变体内层重复）"
        );
    }
}

#[test]
fn zod产物基座判型且六变体放行turn_id() {
    let _lock = lock();
    let zod = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");

    let base = zod_agent_event_base_section(&zod);
    assert!(
        base.contains(TURN_ID_ZOD_BASE),
        "zod 基座缺判型行 {TURN_ID_ZOD_BASE}，实际段:\n{base}"
    );
    assert!(
        !base.contains(TURN_ID_ZOD_VARIANT),
        "基座只判型不放行（放行属六变体，`z.unknown()` 在基座会让类型判据静默失效）"
    );

    let variants = zod_agent_event_variant_section(&zod);
    assert_eq!(
        variants.matches(TURN_ID_ZOD_VARIANT).count(),
        6,
        "六个变体各含一行放行（逐变体一次，不多不少），实际段:\n{variants}"
    );
    // 与既有 seq / timestampMs 放行同式（同一 allowed_object_keys 机制）
    assert_eq!(
        variants.matches("seq: z.unknown().optional(),").count(),
        6,
        "seq 放行同样六处（同式机制对照组）"
    );
    assert_eq!(
        variants
            .matches("timestampMs: z.unknown().optional(),")
            .count(),
        6,
        "timestampMs 放行同样六处"
    );
    // intersection 形态保持
    assert!(
        zod.contains("export const AgentEventSchema = z.strictObject({"),
        "基座 strictObject 形态保持"
    );
    assert!(
        zod.contains("}).and(z.union(["),
        "intersection 形态保持（基座 ∧ 变体联合）"
    );
}

#[test]
fn zod产物轮归属仅限事件模式且不触命令面三表() {
    let _lock = lock();
    let zod = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");

    assert_eq!(
        zod.matches(TURN_ID_ZOD_VARIANT).count(),
        6,
        "放行行全产物恰六处（只在 AgentEventSchema 的六变体）"
    );
    assert_eq!(
        zod.matches("get turnId()").count(),
        2,
        "判型行两处：信封面可空可选 + 轮行面必填（TurnSummarySchema）"
    );
    assert!(
        zod.contains("get turnId() { return z.number(); },"),
        "轮行面 turnId 为必填数字判型（与信封面并存不冲突）"
    );
    // turnId 不进 argsSchemas / resultSchemas / channelArgs（新字段不触命令面）
    for (table, body) in zod_tables(&zod) {
        assert!(
            !body.contains("turnId"),
            "{table} 表不得出现 turnId（信封字段不进命令面）"
        );
    }
}

/// `bindings.ts` 的 `commands` 对象顶层包装名（单制表符缩进，`名称: (...) =>` 形态）。
fn command_wrappers(ts: &str) -> Vec<String> {
    let marker = "export const commands = {";
    let start = ts.find(marker).expect("产物缺 commands 对象") + marker.len();
    let end = ts[start..]
        .find("\n};")
        .map_or(ts.len(), |offset| start + offset);
    ts[start..end]
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix('\t')?;
            if rest.starts_with('\t') || rest.starts_with('*') || rest.starts_with('/') {
                return None;
            }
            let name = rest.split(':').next()?.trim();
            let mut chars = name.chars();
            match chars.next() {
                Some(c) if c.is_ascii_alphabetic() => name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric())
                    .then(|| name.to_owned()),
                _ => None,
            }
        })
        .collect()
}

/// zod 单表顶层键（单制表符缩进的 `get "<命令>"()` 形态；形参位缩进更深）。
fn zod_table_keys(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("\tget \"")?;
            if rest.starts_with('\t') {
                return None;
            }
            Some(rest.split('"').next()?.to_owned())
        })
        .collect()
}

/// 三张命令表各自的键集与表体（`export const <table> = {` 至顶层收尾）。
fn zod_tables(zod: &str) -> [(&'static str, &str); 3] {
    let extract = |table: &'static str| -> (&'static str, &str) {
        let marker = format!("export const {table} = {{");
        let start = zod
            .find(&marker)
            .unwrap_or_else(|| panic!("zod 产物缺 {table} 表"));
        let body_start = start + marker.len();
        let end = zod[body_start..]
            .find("\n} as const;")
            .map_or(zod.len(), |offset| body_start + offset);
        (table, &zod[body_start..end])
    };
    [
        extract("argsSchemas"),
        extract("resultSchemas"),
        extract("channelArgs"),
    ]
}

#[test]
fn zod产物三表键集与命令面一致且禁用片段零命中() {
    let _lock = lock();
    let zod = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");

    let ts = String::from_utf8(fs::read(generated_path()).expect("读取产物应成功"))
        .expect("产物为 UTF-8 文本");

    // 命令面键集（`bindings.ts` 的 `commands` 顶层包装名）与三表键集逐表相等——
    // 新字段只进 `AgentEventSchema`，不增减任何命令面条目。
    let mut expected = command_wrappers(&ts);
    expected.sort();
    assert!(!expected.is_empty(), "命令面非空");
    for (table, body) in zod_tables(&zod) {
        let mut actual = zod_table_keys(body);
        actual.sort();
        assert_eq!(
            actual, expected,
            "{table} 键集与 commands 完全一致（信封新字段不触命令面）"
        );
    }

    // 生成期硬约束（既有断言零改动即成立）：新键未引入禁用写法
    for snippet in [
        "export type ",
        "z.infer",
        "z.object(",
        ".refine(",
        ".default(",
    ] {
        assert!(!zod.contains(snippet), "zod 产物命中禁用片段 {snippet}");
    }
    assert!(
        !zod.contains("turnId: z.number()"),
        "轮归属不得出必填判型（放行行用 z.unknown()）"
    );
    assert!(
        !zod.contains("AgentEventSchema_S"),
        "产物不得含相位伴生名 AgentEventSchema_S*"
    );
}

#[test]
fn zod产物连续两次导出零diff且篡改后恢复仍含轮归属行() {
    let _lock = lock();
    let _restore = RestoreOnDrop;

    let first = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");
    let second = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");
    assert_eq!(first, second, "连续两次导出逐字节零 diff（幂等）");
    assert!(first.contains(TURN_ID_ZOD_BASE), "权威内容含基座判型行");

    // 篡改：抹掉轮归属的判型行与六处放行行 → 重导出恢复权威内容
    let tampered = first
        .replace(TURN_ID_ZOD_BASE, "")
        .replace(TURN_ID_ZOD_VARIANT, "");
    assert_ne!(tampered, first, "前置：篡改生效");
    fs::write(zod_generated_path(), &tampered).expect("篡改 zod 产物应成功");

    export_bindings().expect("重导出应成功");
    let restored = String::from_utf8(fs::read(zod_generated_path()).expect("读取产物应成功"))
        .expect("zod 产物为 UTF-8 文本");
    assert_eq!(restored, first, "篡改不留存（重导出恢复权威内容）");
    assert!(
        restored.contains(TURN_ID_ZOD_BASE),
        "恢复后的内容仍含判型行——该行是产物而非手写残留"
    );
    assert_eq!(
        restored.matches(TURN_ID_ZOD_VARIANT).count(),
        6,
        "恢复后六处放行行齐全"
    );
}

// ---------------------------------------------------------------------------
// 锚定清单换位自证（agent-finalize-tool AC-3）：`SessionToolset` → `FinishSpec`
// ---------------------------------------------------------------------------

/// 换位自证（边界）：本节锚定数组内 `FinishSpec` 恰 1 次、旧枚举名零命中
/// （旧词面残留即判据失效）。
#[test]
fn 反向锚定清单换位自证() {
    let own = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bindings/mod_test.rs"),
    )
    .expect("读取 mod_test.rs");

    let retired_enum = ["Session", "Toolset"].concat();
    // 代码行口径（注释中的历史说明除外）
    let code_only: Vec<&str> = own
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
        })
        .collect();
    let joined = code_only.join("\n");
    let anchor_line = code_only
        .iter()
        .find(|line| line.contains("for engine_side in"))
        .unwrap_or_else(|| panic!("锚定数组行在场"));
    assert!(
        anchor_line.contains("FinishSpec"),
        "锚定数组含 FinishSpec: {anchor_line}"
    );
    assert_eq!(
        anchor_line.matches("\"FinishSpec\"").count(),
        1,
        "FinishSpec 在锚定数组内恰 1 次"
    );
    assert!(
        !anchor_line.contains(&retired_enum),
        "锚定数组零旧枚举名: {anchor_line}"
    );
    assert!(
        !joined
            .replace(&format!("\"{retired_enum}\""), "")
            .contains(&retired_enum)
            || joined.contains("反向锚定清单换位自证"),
        "代码行内仅本自证用例的拼装串出现旧枚举名（其余零命中）"
    );
}

/// 产物面稳定（边界）：同一世代连续两次 `authoritative_snapshot()` 字节逐字
/// 相等（产物是确定性的纯投影，可逐字 diff——`api:check` / `bindings:export`
/// 零 diff 门的前提）。
#[test]
fn 产物面同世代两次快照逐字相等() {
    let _lock = lock();
    let first = authoritative_snapshot();
    let second = authoritative_snapshot();
    assert_eq!(first, second, "同世代两次快照字节逐字相等");

    // 本变更对产物零改动：收口词汇三型全不出线
    let content = String::from_utf8(first).expect("产物为 UTF-8");
    for engine_side in ["FinishPayload", "FinishSpec", "SessionInjections"] {
        assert!(
            !content.contains(engine_side),
            "收口词汇 {engine_side} 不出线（产物零 diff 前提）"
        );
    }
}

// ---------------------------------------------------------------------------
// 文件树面在册（archi-file-tree / AC-3 / AC-14）：四新出线类型与第六命令
// ---------------------------------------------------------------------------

/// 正向（AC-3 / AC-14）：四新类型在两份产物各有具名导出一处；`archiFileTree`
/// 包装签名与两个 zod 表项逐字命中；`ArchiFileEntry` 七字段按声明序出线。
#[test]
fn 文件树四类型与命令在双产物在册() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let zod = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");

    for dto in [
        "ArchiFileEntry",
        "ArchiFileTree",
        "FileEntryKind",
        "FileTreeMark",
    ] {
        assert!(DTO_TYPES.contains(&dto), "DTO 清单应在册 {dto}");
        assert_eq!(
            content.matches(&format!("export type {dto} =")).count(),
            1,
            "bindings.ts 应恰有一处 `export type {dto} =`"
        );
        assert_eq!(
            zod.matches(&format!("export const {dto}Schema =")).count(),
            1,
            "bindings.zod.ts 应恰有一处 `{dto}Schema`"
        );
    }

    // 命令包装：签名与 invoke 名逐字（沿既有产物形状）
    assert!(
        content.contains(
            "archiFileTree: (root: string) => __TAURI_INVOKE<ArchiFileTree>(\"archi_file_tree\", { root })"
        ),
        "archiFileTree 包装签名与 invoke 名逐字"
    );

    // zod 两表：args 与 result 各一表项，形状与导出类型名一致
    let [args, results, channels] = zod_tables(&zod);
    assert!(
        args.1.contains("get \"archiFileTree\"()"),
        "argsSchemas 含 archiFileTree"
    );
    assert!(
        args.1.contains("\"root\": z.string()"),
        "argsSchemas 的 root 判型为字符串"
    );
    assert!(
        results
            .1
            .contains("get \"archiFileTree\"() { return z.lazy(() => ArchiFileTreeSchema); }"),
        "resultSchemas 指向 ArchiFileTreeSchema"
    );
    // `channelArgs` 逐命令在册：该命令无 Channel 形参 ⇒ 表项体为空对象
    let channel_marker = "get \"archiFileTree\"() { return {";
    let channel_start = channels
        .1
        .find(channel_marker)
        .expect("channelArgs 含 archiFileTree 表项")
        + channel_marker.len();
    let channel_body = &channels.1[channel_start..];
    let channel_end = channel_body.find("}; }").expect("表项收尾");
    assert!(
        channel_body[..channel_end].trim().is_empty(),
        "无 Channel 形参 ⇒ 表项体为空对象，实际：{:?}",
        &channel_body[..channel_end]
    );

    // 七字段按声明序出线（`serde_json` 默认保序即契约）
    let entry = type_section(&content, "ArchiFileEntry");
    let keys: Vec<String> = entry
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let name = trimmed.split(':').next()?.trim();
            let plain = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            (plain && !name.is_empty() && name != "export").then(|| name.to_owned())
        })
        .collect();
    assert_eq!(
        keys,
        vec!["path", "name", "kind", "mark", "owners", "contains", "exists"],
        "`ArchiFileEntry` 七字段按声明序出线，实际段：\n{entry}"
    );
    // 单字段扁平：`ArchiFileTree` 恰一个 entries 字段
    let tree_section = type_section(&content, "ArchiFileTree");
    assert!(
        tree_section.contains("entries: ArchiFileEntry[]"),
        "`ArchiFileTree` 单字段 entries 为扁平数组，实际段：\n{tree_section}"
    );
    assert!(
        !tree_section.contains("children"),
        "`ArchiFileTree` 无嵌套字段"
    );
}

/// 边界（AC-14 / 尾锚判据）：既有清单条目零位移——新增项只插在 archi 段尾部，
/// `read_file` / `readFile` 仍是既有 57 条前缀的末条（本变更的通知域五条在其后
/// 追加，全量尾锚随动到 `notification_record`）。
#[test]
fn 文件树登记面尾锚零位移() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert_eq!(
        COMMAND_NAMES[..57].last(),
        Some(&"read_file"),
        "既有 57 条前缀尾锚仍 read_file"
    );
    assert_eq!(
        COMMAND_WRAPPERS[..57].last(),
        Some(&"readFile"),
        "包装清单前缀尾锚仍 readFile"
    );
    assert_eq!(
        COMMAND_NAMES.last(),
        Some(&"notification_record"),
        "全量尾锚随通知域五条追加"
    );
    assert_eq!(
        COMMAND_WRAPPERS.last(),
        Some(&"notificationRecord"),
        "包装清单全量尾锚同序"
    );
    // archi 段在清单中的相邻序：archi_file_tree 紧跟 archi_drift
    let drift_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "archi_drift")
        .expect("archi_drift 在册");
    assert_eq!(
        COMMAND_NAMES.get(drift_index + 1),
        Some(&"archi_file_tree"),
        "archi_file_tree 紧跟 archi_drift（尾部追加序）"
    );
    let drift_wrapper_index = COMMAND_WRAPPERS
        .iter()
        .position(|name| *name == "archiDrift")
        .expect("archiDrift 在册");
    assert_eq!(
        COMMAND_WRAPPERS.get(drift_wrapper_index + 1),
        Some(&"archiFileTree"),
        "archiFileTree 紧跟 archiDrift"
    );

    // 产物面同样持序：两包装在产物中的相对位置随清单
    let drift_pos = content.find("archiDrift:").expect("产物含 archiDrift");
    let file_tree_pos = content
        .find("archiFileTree:")
        .expect("产物含 archiFileTree");
    let read_file_pos = content.find("readFile:").expect("产物含 readFile");
    assert!(
        drift_pos < file_tree_pos && file_tree_pos < read_file_pos,
        "产物包装序：archiDrift < archiFileTree < readFile"
    );
}

/// 正向（AC-4 / AC-14）：`FileTreeMark` 五线值与 `FileEntryKind` 二线值在产物中
/// 逐字命中，且两词表互不串味（`NodeMark` 的 `unfulfilled` 不进文件树词表）。
#[test]
fn 文件树词表线值在产物逐字命中() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let mark_section = type_section(&content, "FileTreeMark");
    for word in [
        "elementRoot",
        "ambiguous",
        "member",
        "containsElements",
        "none",
    ] {
        assert!(
            mark_section.contains(&format!("\"{word}\"")),
            "FileTreeMark 缺线值 {word}，实际段：\n{mark_section}"
        );
    }
    let kind_section = type_section(&content, "FileEntryKind");
    for word in ["\"dir\"", "\"file\""] {
        assert!(
            kind_section.contains(word),
            "FileEntryKind 缺线值 {word}，实际段：\n{kind_section}"
        );
    }
    assert!(
        !mark_section.contains("unfulfilled") && !kind_section.contains("unfulfilled"),
        "文件树词表不得含画布节点标记的词（MUST NOT 复用 NodeMark）"
    );
}

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`agent_answer` 出线面 + `AskMode` / `SessionInjections`
// 反向锚定（引擎侧会话词汇不出线 IPC）
// ---------------------------------------------------------------------------

/// `agent_answer` 出线形态（正向）：生成包装的签名三参（`root` / `sessionId` /
/// `text`）与 invoke 目标 `agent_answer` 逐字；返回 `null`（`Result<(), String>`
/// 的 `()` 出线形态）。
#[test]
fn agent_answer出线签名为三参薄包装() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let expected = "agentAnswer: (root: string, sessionId: string, text: string) => \
                    __TAURI_INVOKE<null>(\"agent_answer\", { root, sessionId, text })";
    let squashed: String = content
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        squashed
            .replace("\n", " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .contains(&expected.split_whitespace().collect::<Vec<_>>().join(" ")),
        "agent_answer 包装签名与 invoke 目标逐字（三参 + null 返回）"
    );
    // 与既有 agent 两命令并存（新增不遮蔽既有出线名）
    for kept in ["agentStart:", "agentStop:"] {
        assert!(content.contains(kept), "既有命令 {kept} 仍在位");
    }
}

/// 反向锚定（异常，AC-9）：`AskMode` 不出线 IPC——产物全文零 `AskMode` 类型名
///（与既有 `SessionInjections` / `FinishSpec` 同口径的反向锚定清单）。
#[test]
fn 引擎侧词汇反向锚定清单含ask_mode() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    for forbidden in [
        "AskMode",
        "SessionInjections",
        "FinishSpec",
        "TurnQuestion",
        "SessionOpen",
        "SessionRef",
    ] {
        assert!(
            !content.contains(forbidden),
            "产物全文零 {forbidden}（引擎侧会话词汇不出线 IPC）"
        );
    }
    // 对照：ask 的**出线**类型仍在（AskPayload 是 IPC 载荷，不是会话词汇）
    assert!(
        content.contains("export type AskPayload ="),
        "AskPayload 仍在产物（ask 中断载荷面）"
    );
}

/// `AskPayload` 两 additive 字段出线形态（正向，AC-9）：`toolCallId` /
/// `sessionId` 为可选可空（`?: string | null`），旧线载荷（无两键）类型层面
/// 仍合法。
#[test]
fn ask_payload两additive字段出线为可选可空() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let start = content
        .find("export type AskPayload = {")
        .expect("AskPayload 类型在场");
    let body = &content[start..(start + 600).min(content.len())];
    assert!(
        body.contains("toolCallId?: string | null"),
        "toolCallId 为可选可空: {body}"
    );
    assert!(
        body.contains("sessionId?: string | null"),
        "sessionId 为可选可空: {body}"
    );
    assert!(body.contains("question: string"), "既有必填 question 不变");
    assert!(body.contains("options: string[]"), "既有必填 options 不变");
}

/// zod 产物同步（正向）：`bindings.zod.ts` 对 `AskPayload` 的两 additive 字段
/// 给 `.optional()` / `.nullable()`，与类型层可选可空同源。
#[test]
fn ask_payload_zod产物两additive字段可选可空() {
    let _lock = lock();
    let zod = std::fs::read_to_string(zod_generated_path()).expect("读取 bindings.zod.ts");
    let ask_block_start = zod
        .find("export const AskPayloadSchema")
        .expect("AskPayloadSchema 段在场");
    let ask_block = &zod[ask_block_start..(ask_block_start + 600).min(zod.len())];
    for field in ["toolCallId", "sessionId"] {
        assert!(
            ask_block.contains(&format!("get {field}()")),
            "zod 产物含 {field} 键"
        );
    }
    assert!(
        ask_block.contains(".nullable().optional()"),
        "两 additive 字段在 zod 侧可空可选（旧线载荷可读）: {ask_block}"
    );
    // 既有必填两键零放宽
    assert!(ask_block.contains("get question() { return z.string(); }"));
    assert!(ask_block.contains("get options() { return z.array(z.string()); }"));
}

/// 生成序（正向，AC-10）：`agentAnswer` 在产物中的偏移紧随 `agentStop`——
/// `all_commands!` 登记序 = 生成序（沿既有三包装相对序先例）；清单侧三命令
/// 连续同序。
#[test]
#[allow(non_snake_case)]
fn 登记面_agentAnswer紧随agentStop生成() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let stop_offset = content.find("agentStop: (").expect("agentStop 包装在场");
    let answer_offset = content
        .find("agentAnswer: (")
        .expect("agentAnswer 包装在场");
    assert!(
        stop_offset < answer_offset,
        "登记序 agentStop 先于 agentAnswer（{stop_offset} < {answer_offset}）"
    );
    // 两者之间只夹 agentAnswer 自己的 JSDoc 注释，无其它命令包装
    let between = &content[stop_offset..answer_offset];
    assert!(
        !between[between.find('\n').expect("agentStop 行有换行")..].contains("__TAURI_INVOKE<"),
        "其间无其它命令包装夹在中间: {between:?}"
    );

    // `all_commands!` 侧同序且连续（invoke 清单内三命令相邻）
    let stop_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "agent_stop")
        .expect("agent_stop 在册");
    let answer_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "agent_answer")
        .expect("agent_answer 在册");
    assert_eq!(
        answer_index,
        stop_index + 1,
        "invoke 清单内 agent_answer 紧随 agent_stop"
    );
    let wrapper_stop = COMMAND_WRAPPERS
        .iter()
        .position(|name| *name == "agentStop")
        .expect("agentStop 在册");
    let wrapper_answer = COMMAND_WRAPPERS
        .iter()
        .position(|name| *name == "agentAnswer")
        .expect("agentAnswer 在册");
    assert_eq!(
        wrapper_answer,
        wrapper_stop + 1,
        "包装清单内 agentAnswer 紧随 agentStop"
    );
}

// ---------------------------------------------------------------------------
// steer-running-agent-step：`agent_steer` 登记面 / 出线面 / 生成序 / 引擎词汇
// 反向锚定
// ---------------------------------------------------------------------------

/// 生成序（正向，AC-8）：`agentSteer` 在产物中的偏移紧随 `agentAnswer`——
/// `all_commands!` 登记序 = 生成序（沿既有相对序先例）；两份清单侧的相邻关系
/// 同源。
#[test]
#[allow(non_snake_case)]
fn 登记面_agentSteer紧随agentAnswer生成() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let answer_offset = content
        .find("agentAnswer: (")
        .expect("agentAnswer 包装在场");
    let steer_offset = content.find("agentSteer: (").expect("agentSteer 包装在场");
    assert!(
        answer_offset < steer_offset,
        "登记序 agentAnswer 先于 agentSteer（{answer_offset} < {steer_offset}）"
    );
    // 两者之间只夹 agentSteer 自己的 JSDoc 注释，无其它命令包装
    let between = &content[answer_offset..steer_offset];
    assert!(
        !between[between.find('\n').expect("agentAnswer 行有换行")..].contains("__TAURI_INVOKE<"),
        "其间无其它命令包装夹在中间: {between:?}"
    );

    let answer_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "agent_answer")
        .expect("agent_answer 在册");
    let steer_index = COMMAND_NAMES
        .iter()
        .position(|name| *name == "agent_steer")
        .expect("agent_steer 在册");
    assert_eq!(
        steer_index,
        answer_index + 1,
        "invoke 清单内紧随 agent_answer"
    );

    let wrapper_answer = COMMAND_WRAPPERS
        .iter()
        .position(|name| *name == "agentAnswer")
        .expect("agentAnswer 在册");
    let wrapper_steer = COMMAND_WRAPPERS
        .iter()
        .position(|name| *name == "agentSteer")
        .expect("agentSteer 在册");
    assert_eq!(
        wrapper_steer,
        wrapper_answer + 1,
        "包装清单内紧随 agentAnswer"
    );
}

/// 签名出线（正向，AC-8）：产物内 `agentSteer` 三参逐字 + invoke 目标
/// `agent_steer` + 返回 `null`（`Result<(), String>` 的出线形态）。
#[test]
fn agent_steer出线签名为三参薄包装() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let expected = "agentSteer: (root: string, sessionId: string, text: string) => \
                    __TAURI_INVOKE<null>(\"agent_steer\", { root, sessionId, text })";
    let squashed = content
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        squashed.contains(&expected.split_whitespace().collect::<Vec<_>>().join(" ")),
        "agent_steer 包装签名与 invoke 目标逐字（三参 + null 返回）"
    );
    // 与既有 agent 命令并存（新增不遮蔽既有出线名）
    for kept in ["agentAnswer:", "agentStart:", "agentStop:"] {
        assert!(content.contains(kept), "既有命令 {kept} 仍在位");
    }
}

/// 引擎面不出线（异常，AC-8 反向锚定）：产物全文零 `SteerMessage` / `SteerRegistry`
/// 类型名——`DTO_TYPES` 清单零补录即覆盖（只出命令包装，不出引擎词汇）。
#[test]
fn 引擎侧词汇反向锚定含steer两类型() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    // 只判「代码面」：命令包装的 JSDoc 会逐字带上 Rust 文档注释（含类型名，
    // 非出线声明）；`export type` / 包装签名里出现才是真正的出线泄漏。
    let code_lines: String = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with('*') || trimmed.starts_with("/*") || trimmed.starts_with("//"))
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    for forbidden in ["SteerMessage", "SteerRegistry"] {
        assert!(
            !code_lines.contains(forbidden),
            "产物代码面零 {forbidden}（引擎侧注入通道词汇不出线 IPC）"
        );
    }
    assert!(
        !DTO_TYPES.iter().any(|name| name.contains("Steer")),
        "DTO 清单零补录（两类型不出线）"
    );
}

// ---------------------------------------------------------------------------
// 本变更：两条新命令的 zod 三表形态（AC-17 / AC-3）
// ---------------------------------------------------------------------------

/// 正向（AC-17 / AC-3）：`bindings.zod.ts` 的三表为两新命令各出一份——`argsSchemas` 的
/// shape 键恰 `root`（`z.string()`）、`resultSchemas` 指向对应 DTO schema、`channelArgs`
/// 为空表（非 Channel 命令）。
#[test]
fn 两新命令的zod三表形态() {
    let _lock = lock();
    let zod = String::from_utf8(zod_authoritative_snapshot()).expect("zod 产物为 UTF-8 文本");

    for (name, result) in [
        ("archiApiCheck", "ArchiApiCheckSchema"),
        ("archiApiGenerate", "ArchiApiWriteOutcomeSchema"),
    ] {
        let args_anchor = format!("\tget \"{name}\"() {{ return z.strictObject({{\n\t\t\t\"root\": z.string(),\n\t\t}}); }},");
        assert!(
            zod.contains(&args_anchor),
            "argsSchemas 的 `{name}` 表项形态（shape 键恰 root）：\n{zod}"
        );
        let result_anchor = format!("\tget \"{name}\"() {{ return z.lazy(() => {result}); }},");
        assert!(
            zod.contains(&result_anchor),
            "resultSchemas 的 `{name}` 指向 {result}"
        );
        let channel_anchor = format!("\tget \"{name}\"() {{ return {{\n\t\t}}; }},");
        assert!(
            zod.contains(&channel_anchor),
            "channelArgs 对 `{name}` 为空表（非 Channel 命令）"
        );
    }

    // 两份产物各含三个新类型的具名导出
    let bindings = String::from_utf8(authoritative_snapshot()).expect("bindings 产物为 UTF-8 文本");
    for name in ["ArchiApiCheck", "ArchiApiMeta", "ArchiApiWriteOutcome"] {
        assert!(
            bindings.contains(&format!("export type {name} =")),
            "bindings.ts 含 `export type {name} =`"
        );
        assert!(
            zod.contains(&format!("const {name}Schema")),
            "bindings.zod.ts 含 `{name}Schema`"
        );
    }

    // 包装签名：(root: string) => __TAURI_INVOKE<T>("archi_api_*", { root })
    for (wrapper, invoke, result) in [
        ("archiApiCheck", "archi_api_check", "ArchiApiCheck"),
        (
            "archiApiGenerate",
            "archi_api_generate",
            "ArchiApiWriteOutcome",
        ),
    ] {
        let signature = format!(
            "{wrapper}: (root: string) => __TAURI_INVOKE<{result}>(\"{invoke}\", {{ root }}),"
        );
        assert!(bindings.contains(&signature), "包装签名为 `{signature}`");
    }
}

// ---------------------------------------------------------------------------
// 目录命令的登记面与出线形参面（AC-7 注册面 / AC-13 出线契约）
// ---------------------------------------------------------------------------

#[test]
fn 目录命令在登记面恰一次且包装同在册() {
    let _lock = lock();

    // 单一登记面（`all_commands!` 一行）：清单内不重不漏——invoke 名走
    // snake_case、生成包装名走 camelCase，两份清单各查各自形态
    let invoke_hits = COMMAND_NAMES
        .iter()
        .filter(|name| **name == "list_model_catalog")
        .count();
    assert_eq!(
        invoke_hits, 1,
        "命令名清单：list_model_catalog 恰一次，实际 {invoke_hits}"
    );
    let wrapper_hits = COMMAND_WRAPPERS
        .iter()
        .filter(|name| **name == "listModelCatalog")
        .count();
    assert_eq!(
        wrapper_hits, 1,
        "包装名清单：listModelCatalog 恰一次，实际 {wrapper_hits}"
    );

    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    assert_eq!(
        content.matches("\"list_model_catalog\"").count(),
        1,
        "产物内 invoke 目标恰一处（单一登记面，不重不漏）"
    );
}

#[test]
fn 目录与effort四类型在出线_dto清单() {
    let _lock = lock();

    for name in [
        "AgentEffortTiers",
        "ModelCatalog",
        "CatalogProtocol",
        "CatalogModel",
    ] {
        assert!(
            DTO_TYPES.contains(&name),
            "出线 DTO 类型清单含 {name}（specta 按结构体名生成）"
        );
    }

    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    for signature in [
        "export type AgentEffortTiers = {",
        "export type ModelCatalog = {",
        "export type CatalogProtocol = {",
        "export type CatalogModel = {",
    ] {
        assert!(content.contains(signature), "产物含类型声明 `{signature}`");
    }
}

#[test]
fn save_agent_provider形参面末位为tuning组而目录命令零形参() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let squashed = content.split_whitespace().collect::<Vec<_>>().join(" ");

    // 八参签名：四项可空调参收成末位一个 `tuning` 组（参数位上限 DD15）
    let save_signature = "saveAgentProvider: (id: number | null, name: string, protocol: \
                          AgentProviderProtocol, baseUrl: string, apiKey: string, models: \
                          AgentModelTiers, tuning: AgentProviderTuning) => \
                          __TAURI_INVOKE<AgentProviderRecord>(\"save_agent_provider\", \
                          { id, name, protocol, baseUrl, apiKey, models, tuning })";
    let expected = save_signature
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        squashed.contains(&expected),
        "saveAgentProvider 形参面与 invoke 目标逐字（末位 tuning 组）"
    );
    assert!(
        squashed.contains("efforts: AgentEffortTiers"),
        "AgentProviderTuning 出线含 efforts 字段"
    );
    assert!(
        squashed.contains("tuning: AgentProviderTuning"),
        "tuning 组为首层唯一末位形参（不平铺四参）"
    );

    // 目录命令零形参：invoke 载荷恒空对象
    let list_signature = "listModelCatalog: () => __TAURI_INVOKE<ModelCatalog>(\
                          \"list_model_catalog\")";
    let expected_list = list_signature
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        squashed.contains(&expected_list),
        "listModelCatalog 零形参（载荷 {}），实际产物: {}",
        "…",
        squashed
            .find("listModelCatalog")
            .map(|index| &squashed[index..index.min(squashed.len())])
            .unwrap_or("")
    );
}

// ---------------------------------------------------------------------------
// 本变更：两条新命令的登记面（`all_commands!` 宏体 → 两份清单 → 生成物）
// ---------------------------------------------------------------------------

/// 正向：`COMMAND_NAMES` 含两新 invoke 名、`COMMAND_WRAPPERS` 含两新包装名
/// （两清单一一对应）。
#[test]
fn 登记面_两新命令进两份清单() {
    for name in ["workspace_config_apply", "list_config_chat_sessions"] {
        assert!(COMMAND_NAMES.contains(&name), "invoke 名补录（{name}）");
        assert_eq!(
            COMMAND_NAMES.iter().filter(|entry| **entry == name).count(),
            1,
            "{name} 恰登记一次"
        );
    }
    for wrapper in ["workspaceConfigApply", "listConfigChatSessions"] {
        assert!(
            COMMAND_WRAPPERS.contains(&wrapper),
            "包装名补录（{wrapper}）"
        );
        assert_eq!(
            COMMAND_WRAPPERS
                .iter()
                .filter(|entry| **entry == wrapper)
                .count(),
            1,
            "{wrapper} 恰登记一次"
        );
    }
    assert_eq!(
        COMMAND_NAMES.len(),
        COMMAND_WRAPPERS.len(),
        "两清单条目数持衡（一一对应）"
    );
}

/// 边界：登记位次——两新命令紧跟同族条目（同族相邻而非尾部追加）。
#[test]
fn 登记面_两新命令与同族相邻() {
    let position = |list: &[&str], name: &str| -> usize {
        list.iter()
            .position(|entry| *entry == name)
            .unwrap_or_else(|| panic!("{name} 在册"))
    };

    let config_position = position(COMMAND_NAMES, "workspace_config_apply");
    assert_eq!(
        config_position,
        position(COMMAND_NAMES, "workspace_config") + 1,
        "workspace_config_apply 紧跟 workspace_config"
    );
    let wrapper_position = position(COMMAND_WRAPPERS, "workspaceConfigApply");
    assert_eq!(
        wrapper_position,
        position(COMMAND_WRAPPERS, "workspaceConfig") + 1,
        "workspaceConfigApply 紧跟 workspaceConfig"
    );

    let list_position = position(COMMAND_NAMES, "list_config_chat_sessions");
    assert_eq!(
        list_position,
        position(COMMAND_NAMES, "list_archi_chat_sessions") + 1,
        "list_config_chat_sessions 紧跟 list_archi_chat_sessions"
    );
    let list_wrapper_position = position(COMMAND_WRAPPERS, "listConfigChatSessions");
    assert_eq!(
        list_wrapper_position,
        position(COMMAND_WRAPPERS, "listArchiChatSessions") + 1,
        "listConfigChatSessions 紧跟 listArchiChatSessions"
    );
}

/// 异常：产物文本副本删去 `listConfigChatSessions` 行 → 持衡断言失败并指认缺项
///（副本驱动，不改产物）。
#[test]
fn 登记面_产物副本缺项被抓() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let broken: String = content
        .lines()
        .filter(|line| !line.contains("listConfigChatSessions:"))
        .collect::<Vec<&str>>()
        .join("\n");

    let missing: Vec<&str> = COMMAND_WRAPPERS
        .iter()
        .copied()
        .filter(|wrapper| {
            content.contains(&format!("{wrapper}:")) && !broken.contains(&format!("{wrapper}:"))
        })
        .collect();
    assert_eq!(
        missing,
        vec!["listConfigChatSessions"],
        "副本删项即被抓（持衡判据指认缺项）"
    );
}

/// 正向：`workspaceConfigApply` 包装行逐字在场（file face 出线），invoke 名与载荷对位。
#[test]
fn 生成物_workspace_config_apply包装逐字() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let line = content
        .lines()
        .find(|line| line.trim_start().starts_with("workspaceConfigApply:"))
        .expect("workspaceConfigApply 包装行在场");
    assert!(
        line.contains("root: string"),
        "第一参为 root: string，实际 {line}"
    );
    assert!(
        line.contains("candidate: Value"),
        "第二参为 file face 的 `Value`（= `export type Value = unknown`），实际 {line}"
    );
    assert!(
        !line.contains("candidate: WorkspaceConfig"),
        "typed `WorkspaceConfig` 入参回潮即红，实际 {line}"
    );
    assert!(
        line.contains("__TAURI_INVOKE<WorkspaceConfigReport>(\"workspace_config_apply\", { root, candidate })"),
        "返回类型 / invoke 命令名 / 载荷逐字，实际 {line}"
    );
    assert!(
        content.contains("export type Value = unknown;"),
        "`Value` 出线别名 = unknown（TS 面 file face）"
    );
}

/// 正向：`listConfigChatSessions` 包装行逐字（三参 + `SessionPage` 返回）。
#[test]
fn 生成物_list_config_chat_sessions包装逐字() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    assert!(
        content.contains(
            "listConfigChatSessions: (root: string, offset: number, limit: number) => \
             __TAURI_INVOKE<SessionPage>(\"list_config_chat_sessions\", { root, offset, limit })"
        ),
        "包装形态不符（三参 / 返回类型 / invoke 命令名 / 载荷）"
    );
}

/// 异常：产物副本把 `candidate: Value` 改成 typed `WorkspaceConfig` → 断言失败并指认
///（file face typed 入参禁令的机械锚）。
#[test]
fn 生成物_typed入参副本被抓() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");
    let broken = content.replace("candidate: Value", "candidate: WorkspaceConfig");

    let line = broken
        .lines()
        .find(|line| line.trim_start().starts_with("workspaceConfigApply:"))
        .expect("包装行仍在（只改类型）");
    assert!(
        line.contains("candidate: WorkspaceConfig"),
        "副本改写生效（判据面可观测）"
    );
    assert!(
        !content
            .lines()
            .any(|line| line.contains("candidate: WorkspaceConfig")),
        "权威产物零 typed 入参（副本与产物的差异即判据）"
    );
}

/// zod 产物中某命令的入参 `strictObject` 块文本（`get "<name>"() { return z.strictObject({ … }); },`）。
fn zod_args_block<'a>(content: &'a str, name: &str) -> &'a str {
    let marker = format!("get \"{name}\"() {{ return z.strictObject({{");
    let start = content
        .find(&marker)
        .unwrap_or_else(|| panic!("zod 产物缺命令入参块 {name}"));
    let tail = &content[start..];
    let end = tail
        .find("}); },")
        .map_or(tail.len(), |offset| offset + "}); },".len());
    &tail[..end]
}

/// `workspaceConfigApply` 入参块的判据（正向 / 负向副本公用：异常形态只在内存副本上
/// 删 getter 驱动，不改生成物）。
fn assert_workspace_config_apply_args(block: &str) {
    assert!(
        block.contains("\"root\": z.string()"),
        "入参含 root getter：{block}"
    );
    assert!(
        block.contains("\"candidate\": z.unknown()"),
        "candidate 沿用既有 unknown 出线形态（z.unknown()）：{block}"
    );
}

/// 正向（zod 面）：两新命令的入参 schema 各自落在同一 `strictObject` 块内，
/// `candidate` 为 `z.unknown()`。
#[test]
fn zod产物_两新命令schema齐备() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();

    let apply_args = zod_args_block(&content, "workspaceConfigApply");
    assert_workspace_config_apply_args(apply_args);

    let list_args = zod_args_block(&content, "listConfigChatSessions");
    for field in ["\"root\": z.string()", "\"offset\":", "\"limit\":"] {
        assert!(
            list_args.contains(field),
            "入参缺 {field} getter：{list_args}"
        );
    }
    assert_eq!(
        list_args.matches("z.int()").count(),
        2,
        "offset / limit 为 u32 整数面：{list_args}"
    );
}

/// 异常（zod 面）：产物副本删去 `candidate` getter → 判据被抓且文案指认缺键
///（zod 面漏字段 = 前端解析面漏字段；副本驱动，不改产物）。
#[test]
fn zod产物_缺candidate的副本被抓且文案指认缺键() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();
    let apply_args = zod_args_block(&content, "workspaceConfigApply");

    let without_candidate = without_line(apply_args, "\"candidate\": z.unknown(),");
    assert!(
        !without_candidate.contains("\"candidate\""),
        "副本改写生效（判据面可观测）：{without_candidate}"
    );

    let text = panic_text(|| assert_workspace_config_apply_args(&without_candidate));
    assert!(
        text.contains("candidate"),
        "删 candidate getter 的副本应被断言抓且指认缺键，实际文案 {text:?}"
    );
}

/// 正向（zod 返回面）：两新命令的结果 schema 指向对应报告类型（`WorkspaceConfigReportSchema`
/// / `SessionPageSchema`）。
#[test]
fn zod产物_两新命令返回schema指向报告类型() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();

    assert!(
        content.contains(
            "get \"workspaceConfigApply\"() { return z.lazy(() => WorkspaceConfigReportSchema); }"
        ),
        "workspaceConfigApply 返回 schema 指向 WorkspaceConfigReportSchema"
    );
    assert!(
        content.contains(
            "get \"listConfigChatSessions\"() { return z.lazy(() => SessionPageSchema); }"
        ),
        "listConfigChatSessions 返回 schema 指向 SessionPageSchema"
    );
}

// ===========================================================================
// 关系详情面（`relation-api-dependency-check` / AC-10 / D22）
//
// 生成物零手改：新类型与新命令只来自 `bindings:export` 重导。判据面是「清单在册 +
// 产物逐名含」，清单顺序按字典序（`ArchiRelation` 之后紧跟 `ArchiRelationDetail`）。
// ===========================================================================

/// 正向：`ARCHI_DTO_TYPES` 字典序中 `ArchiRelationDetail` 紧随 `ArchiRelation`，
/// 且两者都在产物中出线。
#[test]
fn 生成物_关系详情类型紧随关系类型在册() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let index = ARCHI_DTO_TYPES
        .iter()
        .position(|name| *name == "ArchiRelation")
        .expect("ArchiRelation 在册");
    assert_eq!(
        ARCHI_DTO_TYPES[index + 1],
        "ArchiRelationDetail",
        "字典序紧随关系类型（清单序即可读性锚）"
    );
    for name in ["ArchiRelation", "ArchiRelationDetail"] {
        assert!(
            content.contains(&format!("export type {name} =")),
            "产物缺 `export type {name} =`"
        );
    }
    assert!(
        content.contains("archiRelation: (root: string, from: string, to: string)"),
        "产物含三条参数的 `archiRelation` 包装"
    );
    assert!(
        content.contains("\"archi_relation\""),
        "产物含 invoke 命令名 `archi_relation`"
    );
}

/// 边界：`ArchiRelation` 的 `apis` 为末尾第六字段（前五字段零位移），`ArchiRelationDetail`
/// 十二字段按声明序出线（两桶 camelCase）。
#[test]
fn 生成物_关系与详情字段面() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let relation = type_section(&content, "ArchiRelation");
    assert_eq!(
        field_names(&relation),
        vec!["from", "to", "description", "mark", "reason", "apis"],
        "关系投影六字段（`apis` 末位追加）"
    );
    assert!(
        relation.contains("apis: string[]"),
        "`apis` 出线为非可选字符串数组：\n{relation}"
    );

    let detail = type_section(&content, "ArchiRelationDetail");
    assert_eq!(
        field_names(&detail),
        vec![
            "from",
            "to",
            "description",
            "mark",
            "reason",
            "modelFile",
            "modelLine",
            "declared",
            "used",
            "usedNotDeclared",
            "declaredNotUsed",
            "occurrences",
        ],
        "关系详情十二字段按声明序出线（两桶 camelCase）"
    );
    assert!(
        detail.contains("occurrences: ArchiDriftOccurrence[]"),
        "命中点复用既有聚集类型（零新嵌套类型）"
    );
}

/// 组合：清单与产物双向自洽——清单每条都在产物命中，产物里的 `ArchiRelation*` 类型
/// 也都已在清单登记（不做单侧信任）。
#[test]
fn 生成物_关系面清单与产物双向自洽() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for name in ARCHI_DTO_TYPES {
        assert!(
            content.contains(&format!("export type {name} =")),
            "清单在册但产物缺声明：{name}"
        );
    }
    for name in ["ArchiRelation", "ArchiRelationDetail"] {
        assert!(
            DTO_TYPES.contains(&name),
            "产物有该类型而总清单漏登：{name}"
        );
    }
}

// ---------------------------------------------------------------------------
// 通知域（desktop-notification-hub）：五命令按登记序追加、出线载荷零读状态、
// zod 三表项齐备、`all_commands!` 登记面锚
// ---------------------------------------------------------------------------

/// 正向：五命令 / 五包装按登记序落在清单末尾，既有前缀零位移（首锚与 57 条
/// 前缀逐条不变）。
#[test]
fn 通知五命令按登记序追加且既有前缀零位移() {
    assert_eq!(
        &COMMAND_NAMES[COMMAND_NAMES.len() - 5..],
        &[
            "notification_subscribe",
            "notification_list",
            "notification_mark_read",
            "notification_mark_all_read",
            "notification_record",
        ],
        "invoke 名按 all_commands! 登记序追加"
    );
    assert_eq!(
        &COMMAND_WRAPPERS[COMMAND_WRAPPERS.len() - 5..],
        &[
            "notificationSubscribe",
            "notificationList",
            "notificationMarkRead",
            "notificationMarkAllRead",
            "notificationRecord",
        ],
        "包装名与 invoke 名同序"
    );
    assert_eq!(
        COMMAND_NAMES.len(),
        COMMAND_WRAPPERS.len(),
        "两清单长度恒相等"
    );

    // 既有前缀零位移：前 57 条的首锚与尾锚不变（通知域只追加不改写）
    assert_eq!(COMMAND_NAMES[..57].first(), Some(&"list_changes"));
    assert_eq!(COMMAND_NAMES[..57].last(), Some(&"read_file"));
    assert_eq!(COMMAND_WRAPPERS[..57].last(), Some(&"readFile"));
}

/// 异常：`all_commands!` 登记面反向锚——`notification_` 前缀命令名恰 5 处且零
/// 重复、`pub mod notifications;` 恰一处（漏登记即命令不可达）。
#[test]
fn commands登记面通知命令恰五处且模块登记恰一处() {
    let source = include_str!("../commands/mod.rs");

    // 模块路径 `commands::notifications::…` 不含 `notification_` 下划线形态，
    // 故计数即命令名出现次数
    assert_eq!(
        source.match_indices("notification_").count(),
        5,
        "五条通知命令逐条登记"
    );

    let names = [
        "notification_subscribe",
        "notification_list",
        "notification_mark_read",
        "notification_mark_all_read",
        "notification_record",
    ];
    for name in names {
        // 每条命令名在宏清单里恰一次（`notification_mark_read` 与
        // `notification_mark_all_read` 互不包含）
        let pattern = format!("::{name},");
        assert_eq!(
            source.matches(&pattern).count(),
            1,
            "命令名 {name} 应恰登记一次"
        );
    }
    assert_eq!(
        source.matches("pub mod notifications;").count(),
        1,
        "模块登记恰一处"
    );
}

/// 正向 / 边界：`Notification` 载荷类型体零读状态、`NotificationLevel` 联合恰四
/// 字面量（变体序冻结的产物锚）。
#[test]
fn 通知载荷出线八字段零读状态且级别联合四字面量() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let section = type_section(&content, "Notification");
    assert_eq!(
        field_names(&section),
        vec![
            "id",
            "kind",
            "level",
            "message",
            "body",
            "root",
            "sourceRef",
            "createdAt",
        ],
        "八字段按声明序出线"
    );
    assert!(!section.contains("readAt"), "载荷不含读状态（camelCase）");
    assert!(!section.contains("read_at"), "载荷不含读状态（线名）");

    let level = type_section(&content, "NotificationLevel");
    for literal in ["\"info\"", "\"success\"", "\"warning\"", "\"error\""] {
        assert!(level.contains(literal), "级别联合缺字面量 {literal}");
    }
    assert_eq!(
        level.matches('"').count(),
        8,
        "恰四字面量（两条引号/字面量）"
    );

    let page = type_section(&content, "NotificationPage");
    assert_eq!(field_names(&page), vec!["items", "total"], "分页信封两字段");
}

/// 正向：zod 三表（argsSchemas / resultSchemas / channelArgs）各含五新命令键。
#[test]
fn 通知五命令的zod三表项齐备() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();

    let wrappers = [
        "notificationSubscribe",
        "notificationList",
        "notificationMarkRead",
        "notificationMarkAllRead",
        "notificationRecord",
    ];
    for wrapper in wrappers {
        assert_eq!(
            content.matches(&format!("get \"{wrapper}\"()")).count(),
            3,
            "{wrapper} 应在参数 / 结果 / 信道三表各有一项"
        );
    }
    assert!(
        content.contains("NotificationFeedbackInputSchema"),
        "反馈入参 schema 随批生成"
    );
}

// ---------------------------------------------------------------------------
// session-preamble-snapshot：生成产物出线面（`SessionPreambleInfo` 类型 +
// `SessionSummary.preamble` 可空 + zod 镜像随动；零新增命令、零手改）
// ---------------------------------------------------------------------------

#[test]
fn bindings快照类型段两字段与session_summary的preamble可空段逐字() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    let preamble = type_section(&content, "SessionPreambleInfo");
    assert!(
        preamble.contains("savedAt: number,"),
        "拍存时刻出线为 number，实际: {preamble}"
    );
    assert!(
        preamble.contains("content: string,"),
        "正文出线为 string，实际: {preamble}"
    );
    let field_lines: Vec<&str> = preamble
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty() && !line.starts_with("/**") && !line.starts_with('*') && *line != "*/"
        })
        .collect();
    assert_eq!(
        field_lines,
        vec![
            "export type SessionPreambleInfo = {",
            "savedAt: number,",
            "content: string,",
            "};"
        ],
        "类型段恰两字段、无多余键"
    );

    let summary = type_section(&content, "SessionSummary");
    assert!(
        summary.contains("preamble: SessionPreambleInfo | null,"),
        "快照位可空出线，实际: {summary}"
    );
    assert!(
        summary.contains("row: SessionRow,"),
        "三件套零改动: {summary}"
    );
    assert!(
        summary.contains("stats: SessionStats,"),
        "三件套零改动: {summary}"
    );
    assert!(
        summary.contains("turns: TurnSummary[],"),
        "三件套零改动: {summary}"
    );
}

#[test]
fn bindings命令面零新增且session_detail包装与既有三段逐字不变() {
    let _lock = lock();
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    for absent in [
        "session_preamble\"",
        "session_preamble_reload",
        "session_preamble_status",
        "sessionPreamble",
    ] {
        assert!(
            !content.contains(absent),
            "命令面零新增：产物不应含 {absent}"
        );
    }
    assert!(
        content.contains(
            "sessionDetail: (root: string, sessionId: string) => __TAURI_INVOKE<SessionSummary | null>(\"session_detail\", { root, sessionId })"
        ),
        "session_detail 包装逐字不变（只扩载荷、不改命令面）"
    );
    for name in COMMAND_NAMES {
        assert!(
            content.contains(&format!("\"{name}\"")),
            "既有命令名保持：{name}"
        );
    }
    assert!(
        !content.contains("Result<"),
        "产物无 Result 包装（Throw 模式回归）"
    );

    // 其余三件套类型段逐字不变（新字段只在 SessionSummary 上追加）
    for (name, expected) in [
        ("SessionRow", "remoteSessionId: string | null,"),
        ("SessionStats", "totalDurationMs: number | null,"),
        ("TurnSummary", "costUsd: number | null,"),
    ] {
        assert!(
            type_section(&content, name).contains(expected),
            "{name} 类型段逐字不变（期望含 {expected}）"
        );
    }
}

#[test]
fn bindings产物确定性_连续两次导出逐字节相等() {
    let _lock = lock();
    let first = authoritative_snapshot();
    let second = authoritative_snapshot();
    assert_eq!(first, second, "连续两次导出逐字节相等（零 diff 门）");

    let first_zod = authoritative_zod_snapshot();
    let second_zod = authoritative_zod_snapshot();
    assert_eq!(first_zod, second_zod, "zod 产物同样确定性");
}

#[test]
fn bindings产物篡改两形态被断言抓且文案指认() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let content = String::from_utf8(authoritative_snapshot()).expect("产物为 UTF-8 文本");

    // 形态一：删 preamble 行
    let summary = type_section(&content, "SessionSummary");
    let without_preamble = without_line(&summary, "preamble: SessionPreambleInfo | null,");
    let text = panic_text(|| assert_session_summary_preamble(&without_preamble));
    assert!(
        text.contains("preamble: SessionPreambleInfo | null,"),
        "删 preamble 行的副本应被断言抓且指认缺项，实际文案 {text:?}"
    );

    // 形态二：类型改名为 SessionPreamble
    let renamed = type_section(&content, "SessionPreambleInfo").replace(
        "export type SessionPreambleInfo =",
        "export type SessionPreamble =",
    );
    let text = panic_text(|| assert_session_preamble_type(&renamed));
    assert!(
        text.contains("SessionPreambleInfo"),
        "改名副本应被断言抓且指认缺项，实际文案 {text:?}"
    );
}

/// `SessionSummary` 类型段的快照位判据（篡改用例的断言体）。
fn assert_session_summary_preamble(section: &str) {
    assert!(
        section.contains("preamble: SessionPreambleInfo | null,"),
        "SessionSummary 段缺 `preamble: SessionPreambleInfo | null,`，实际: {section}"
    );
}

/// `SessionPreambleInfo` 类型段的字段判据（篡改用例的断言体）。
fn assert_session_preamble_type(section: &str) {
    assert!(
        section.contains("export type SessionPreambleInfo ="),
        "产物缺 `export type SessionPreambleInfo =`，实际: {section}"
    );
    assert!(
        section.contains("savedAt: number,") && section.contains("content: string,"),
        "SessionPreambleInfo 段缺字段，实际: {section}"
    );
}

#[test]
fn zod镜像快照schema与summary的preamble可空getter() {
    let _lock = lock();
    let content = authoritative_zod_snapshot();

    let schema = zod_section(&content, "SessionPreambleInfoSchema");
    assert!(
        schema.contains("z.strictObject("),
        "快照 schema 为 strictObject（缺键 / 多键即拒），实际: {schema}"
    );
    assert!(
        schema.contains("get savedAt() { return z.number(); }"),
        "savedAt getter 形态，实际: {schema}"
    );
    assert!(
        schema.contains("get content() { return z.string(); }"),
        "content getter 形态，实际: {schema}"
    );

    let summary = zod_section(&content, "SessionSummarySchema");
    assert!(
        summary.contains("get preamble() { return SessionPreambleInfoSchema.nullable(); }"),
        "summary 的快照位为 nullable 镜像，实际: {summary}"
    );
    assert!(
        content.contains(
            "get \"sessionDetail\"() { return z.lazy(() => SessionSummarySchema).nullable(); }"
        ),
        "命令映射表指向同一 schema（零运行期耦合）"
    );
    assert!(
        !content.contains("from \"./bindings\""),
        "zod 产物不 import bindings.ts（零运行期耦合）"
    );
}

#[test]
fn zod镜像快照schema篡改三形态被断言抓且指认() {
    let _lock = lock();
    let _restore = RestoreOnDrop;
    let content = authoritative_zod_snapshot();
    let summary = zod_section(&content, "SessionSummarySchema");

    // 形态一：删 preamble getter
    let without_getter = without_line(&summary, "get preamble()");
    let text = panic_text(|| assert_summary_preamble_getter(&without_getter));
    assert!(
        text.contains("get preamble()"),
        "删 getter 的副本应被抓且指认，实际文案 {text:?}"
    );

    // 形态二：去掉 .nullable()
    let non_nullable = summary.replace(
        "get preamble() { return SessionPreambleInfoSchema.nullable(); }",
        "get preamble() { return SessionPreambleInfoSchema; }",
    );
    let text = panic_text(|| assert_summary_preamble_getter(&non_nullable));
    assert!(
        text.contains("nullable()"),
        "去掉 .nullable() 的副本应被抓且指认，实际文案 {text:?}"
    );

    // 形态三：content 改成 z.number()
    let schema = zod_section(&content, "SessionPreambleInfoSchema").replace(
        "get content() { return z.string(); }",
        "get content() { return z.number(); }",
    );
    let text = panic_text(|| assert_preamble_schema_fields(&schema));
    assert!(
        text.contains("z.string()"),
        "content 改成 number 的副本应被抓且指认，实际文案 {text:?}"
    );
}

/// zod 快照 schema 的字段判据（篡改用例的断言体）。
fn assert_preamble_schema_fields(section: &str) {
    assert!(
        section.contains("get content() { return z.string(); }"),
        "SessionPreambleInfoSchema 缺 `get content() {{ return z.string(); }}`，实际: {section}"
    );
}

/// zod summary 的快照位判据（篡改用例的断言体）。
fn assert_summary_preamble_getter(section: &str) {
    assert!(
        section.contains("get preamble() { return SessionPreambleInfoSchema.nullable(); }"),
        "SessionSummarySchema 缺 `get preamble() {{ return SessionPreambleInfoSchema.nullable(); }}`，实际: {section}"
    );
}
