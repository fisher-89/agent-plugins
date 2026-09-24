/**
 * 查询 DTO 与 ArtifactEnvelope 的 TS 镜像类型（对齐 Rust serde camelCase 序列化）。
 */

export type Inventory = 'v2' | 'v1' | 'v0';

type Verdict = 'pass' | 'fail';

type ChangeSource = 'active' | 'archive';

type FileLogOp = 'write' | 'delete' | 'revert';

export interface ChecklistItem {
  item: string;
  pass: boolean;
  evidence: string;
}

export interface ChangeSummary {
  name: string;
  source: ChangeSource;
  inventory: Inventory;
  created: string | null;
  unparsable: boolean;
}

interface ArchiveGroup {
  /** 形如 "2026-05"；null 即"未知时间"组（固定排序列表尾） */
  month: string | null;
  changes: ChangeSummary[];
}

export interface ChangeList {
  active: ChangeSummary[];
  archiveGroups: ArchiveGroup[];
}

export interface AttemptRecord {
  attempt: number | null;
  verdict: Verdict;
  report: string;
  checklist: ChecklistItem[];
  skipped: boolean;
  stale: boolean;
  startAt: string | null;
  timestamp: string | null;
  backtrackTo: string | null;
  backtrackReason: string | null;
}

export interface PhaseEntry {
  phase: string;
  attempts: AttemptRecord[];
}

interface ActivePhase {
  phase: string;
  attempt: number;
  startAt: string | null;
}

interface InterruptedEntry {
  phase: string;
  attempt: number;
  startAt: string | null;
  endAt: string | null;
}

interface FileLogEntry {
  op: FileLogOp;
  scope: string;
  attempt: number | null;
  path: string;
  at: string | null;
}

export interface ArtifactDescriptor {
  kind: string;
  source: string;
  title: string;
}

export interface ChangeDetail {
  name: string;
  source: ChangeSource;
  inventory: Inventory;
  created: string | null;
  unparsable: boolean;
  pipeline: PhaseEntry[];
  activePhase: ActivePhase | null;
  interrupted: InterruptedEntry[];
  fileLog: FileLogEntry[] | null;
  artifacts: ArtifactDescriptor[];
}

/** 信封契约前端侧；payload 为 kind 自描述结构，由各 renderer 自行收窄 */
export interface ArtifactEnvelope {
  kind: string;
  version: number;
  title: string;
  payload: unknown;
  fallbackText: string | null;
}

/** workspace 清单记录（对齐 store::WorkspaceRecord 的 serde camelCase 序列化） */
export interface WorkspaceRecord {
  /** canonical 完整路径（库内 key） */
  root: string;
  /** 目录名最后一段（展示用） */
  name: string;
  /** 入库时间（UTC unix 毫秒） */
  addedAt: number;
}

// ---------------------------------------------------------------------------
// agent 域 DTO（对齐 core/agent 信封与 store::AgentRunRecord 的 serde camelCase
// 线格式；线格式 = 落库形态 = 本文件镜像基准）
// ---------------------------------------------------------------------------

/** 环境档位双档：default 完整环境 / bare 纯净档（须外部认证前提） */
export type AgentEnvMode = 'default' | 'bare';

/** permission-mode 三档 */
export type AgentPermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions';

/** run 状态受控字符串 */
export type AgentRunStatus = 'running' | 'completed' | 'failed';

/** 消息内块四变体（tag `kind`，camelCase） */
export type AgentBlock =
  | { kind: 'text'; text: string }
  | { kind: 'thinking'; thinking: string }
  | { kind: 'toolUse'; id: string; name: string; input: unknown }
  | { kind: 'toolResult'; id: string; content: string; isError: boolean };

/** 事件公共字段：seq 为单调序号（入库排序键），timestampMs 为盖戳时刻 */
interface AgentEventBase {
  seq: number;
  timestampMs: number;
}

/** 事件信封五变体（kind 判别 union；`kind` 扁平于线格式顶层） */
export type AgentEvent =
  | (AgentEventBase & {
      kind: 'runStarted';
      model: string | null;
      sessionId: string | null;
      tools: string[];
      mcpServers: string[];
    })
  | (AgentEventBase & {
      kind: 'message';
      role: string;
      blocks: AgentBlock[];
      parentToolUseId: string | null;
    })
  | (AgentEventBase & { kind: 'systemNotice'; subtype: string; payload: unknown })
  | (AgentEventBase & {
      kind: 'runResult';
      subtype: string;
      isError: boolean;
      numTurns: number | null;
      durationMs: number | null;
      costUsd: number | null;
      usage: unknown;
      sessionId: string | null;
    })
  | (AgentEventBase & { kind: 'raw'; eventType: string; rawJson: string });

/** agent 运行记录（对齐 store::AgentRunRecord 的 serde camelCase 序列化） */
export interface AgentRunRecord {
  id: number;
  prompt: string;
  cwd: string;
  env: AgentEnvMode;
  permissionMode: AgentPermissionMode;
  status: AgentRunStatus;
  startedAt: number;
  finishedAt: number | null;
  numTurns: number | null;
  costUsd: number | null;
  durationMs: number | null;
  sessionId: string | null;
  error: string | null;
}

// ---------------------------------------------------------------------------
// db 查看域 DTO（对齐 store 信封类型的 serde camelCase 序列化；信封值为 JSON
// 值，native_db 类型不越信封）
// ---------------------------------------------------------------------------

/** 模型清单一行：模型名 + 记录计数（对齐 store::ModelInfo） */
export interface ModelInfo {
  name: string;
  count: number;
}

/** 记录信封：key / value 均为 JSON 值（对齐 store::RecordEnvelope） */
export interface RecordEnvelope {
  key: unknown;
  value: unknown;
}
