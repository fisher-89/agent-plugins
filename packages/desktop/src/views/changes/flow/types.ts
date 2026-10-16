import type { ArtifactEnvelope, ChangeStepKind, ChangeStepStatus, RunEdgeKind } from '@/types/dto';

/**
 * 边类型：直接读后端边表
 */
export type FlowEdgeKind = RunEdgeKind;

interface FlowNodeBase {
  id: string;
  phase: string;
  /** attempt 缺号按 0 兜底（与后端 detail.rs 的 unwrap_or(0) 同款） */
  attempt: number;
  colIndex: number;
  /** 列内归并序（0 起）：节点相对坐标的执行序项 */
  order: number;
  /** 所属列容器 id `col:<phase>`（react-flow subflow 父节点） */
  parentId: string;
}

/**
 * 运行步节点四分类词汇（runStepNode 节点类型徽章可辨）：WorkerAgent 五角色 /
 * ToolStep 命令 / Git 四步 / Gate 三门。
 */
export type RunStepGroup = 'workerAgent' | 'toolStep' | 'git' | 'gate';

/**
 * role 标签（WorkerRole 线格式，与后端 sourceRef 定式第三段一致）：
 * sourceRef = `<id>/<phase>/<role>/<attempt>`（身份段恒 change id，D3），
 * 会话转录联动按此 exact-match 反查 agentSessions（source='change'）；归档两
 * agent 步以 `spec_sync` / `resolve_conflicts` 为第三段（节点 sessionId 直查优先）。
 */
export type FlowRoleLabel =
  | 'executor'
  | 'evaluator'
  | 'decision'
  | 'spec_sync'
  | 'resolve_conflicts';

/**
 * 转录联动寻址键
 */
export interface RoleSessionRef {
  role: FlowRoleLabel;
  sessionId: string | null;
}

/** 运行步节点 */
export interface RuntimeFlowNode extends FlowNodeBase {
  kind: 'runtime';
  /** 归属 run id（节点 id 第一身份段——重启 / 中断重跑同 attempt 的跨 run
   * 同名身份键不再串扰） */
  runId: string;
  runStepKind: ChangeStepKind;
  group: RunStepGroup;
  role: FlowRoleLabel | null;
  /** 同 (phase, attempt, step) 内的执行实例号（后端稳定身份键第四段） */
  instanceNo: number;
  status: ChangeStepStatus;
  sessionId: string | null;
  detail: string | null;
  /** 全量输出引用（ExecOutput::Recorded 的 exec_id；无记录恒 null） */
  outputRef: string | null;
  /** 步开始时刻（UTC unix 毫秒；旧库行 0 = 缺席，判定收 metrics 模块单点） */
  startedAt: number;
  /** 步收口时刻（UTC unix 毫秒；running 收口在途步 null） */
  endedAt: number | null;
  /** 本步 turn token 增量（仅 WorkerAgent 步终态有值；缺席 null） */
  tokenInput: number | null;
  /** 本步 turn token 增量（输出向；口径同 tokenInput） */
  tokenOutput: number | null;
  /** 活面来源标注（D7）：统一视图合成时按源落面——活面（在飞 run）节点 true、
   * 库读史节点 false；走秒仅 live && running 挂载（库读史 running 残留不走秒） */
  live: boolean;
}

/** 图节点单分类：仅 runtime step 节点 */
export type FlowNode = RuntimeFlowNode;

/** phase 列容器（10 站恒定，未走的站呈现空列） */
export interface FlowColumn {
  id: string;
  phase: string;
  colIndex: number;
}

export interface FlowEdge {
  id: string;
  source: string;
  target: string;
  kind: FlowEdgeKind;
  /** 回跳边取发起相位最近一次条目的 backtrackReason；其余边为 null */
  label: string | null;
}

export interface FlowGraph {
  columns: FlowColumn[];
  /** runtime step 节点按统一视图序排列（列容器经 columns 单列承载） */
  nodes: FlowNode[];
  edges: FlowEdge[];
}

/**
 * 挂载后的过程素材（单层：文档 + eval-checklist 均挂列）：`columnDocs` 键 = 列 id
 * `col:<phase>`，值为该站过程文档（`markdown-doc` / `tasks-progress`）与
 * eval-checklist 信封（挂列头后经 `ArtifactTabs` 渲染全套元数据）。
 */
export interface FlowMaterials {
  columnDocs: Record<string, ArtifactEnvelope[]>;
}

/** 抽屉选中对象：列头（按 phase）与 runtime step 节点（按节点 id）共用同一交互入口 */
export type DrawerSelection =
  | { scope: 'column'; phase: string }
  | { scope: 'node'; nodeId: string };
