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
  /** 最近打开时间（UTC unix 毫秒），清单排序依据 */
  lastOpenedAt: number;
}
