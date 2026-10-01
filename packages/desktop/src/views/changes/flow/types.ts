/**
 * 流程图视图模型类型（运行时派生物，不持久化；唯一事实源是经 get_change_detail
 * 取得的 ChangeDetail DTO）。零 react / 零 @xyflow/react / 零 invoke 依赖。
 *
 * 节点 id 方案（graph.ts 生成；素材挂载与抽屉选中按 phase / attempt 字段定位，
 * 不在挂载层重复拼写 id）：
 * - 列容器       `col:<phase>`
 * - eval 节点    `eval:<phase>:<attempt>`（attempt 缺号按 0 兜底，同 id 撞车时加序号后缀）
 * - active 节点  `active:<phase>:<attempt>`
 * - interrupted  `interrupted:<phase>:<attempt>`
 * - run 步节点   `run:<phase>:<attempt>:<step>[:<seq>]`（seq 仅同键重复步歧义时追加，
 *                如 static-check 反馈边多次迭代；running → 终态同键归并同节点）
 * - 边           `edge:<source>-><target>`
 */
import type {
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
  ChangeStepKind,
  ChangeStepStatus,
} from '../../../types/dto';

/**
 * file_log 条目前端镜像：由 ChangeDetail['fileLog'] 派生（dto 不导出该条目类型，
 * 派生即可拿到与线格式同源的结构，dto.ts 零改动）。
 */
export type FileLogEntry = NonNullable<ChangeDetail['fileLog']>[number];

/** 事件节点三分类 */
export type FlowNodeKind = 'eval' | 'active' | 'interrupted';

/** 边类型：由两端列索引差 Δ 的符号派生（>0 前进 / =0 重试 / <0 回跳） */
export type FlowEdgeKind = 'forward' | 'retry' | 'backtrack';

interface FlowNodeBase {
  id: string;
  phase: string;
  /** attempt 缺号按 0 兜底（与后端 detail.rs 的 unwrap_or(0) 同款） */
  attempt: number;
  colIndex: number;
  /** 列内归并序（0 起）：事件节点相对坐标的执行序项 */
  order: number;
  /** 所属列容器 id `col:<phase>`（react-flow subflow 父节点） */
  parentId: string;
}

/** eval 事件：来自 pipeline 各站的 attempts[]，record 原样携带（verdict / report / checklist 等） */
export interface EvalFlowNode extends FlowNodeBase {
  kind: 'eval';
  record: AttemptRecord;
}

/** active 事件：来自 activePhase，运行中无 verdict，位于所在列的接流末端 */
export interface ActiveFlowNode extends FlowNodeBase {
  kind: 'active';
  startAt: string | null;
}

/** interrupted 事件：来自 interrupted[]，dashed 灰显留档；独立成节点不与同号 eval 合并 */
export interface InterruptedFlowNode extends FlowNodeBase {
  kind: 'interrupted';
  startAt: string | null;
  endAt: string | null;
}

/**
 * 运行步节点三分类词汇（runStepNode 节点类型徽章可辨）：WorkerAgent 三角色 /
 * ToolStep 四命令 / Gate 三门。
 */
export type RunStepGroup = 'workerAgent' | 'toolStep' | 'gate';

/**
 * role 标签（WorkerRole 线格式，与后端 sourceRef 定式第三段一致）：
 * sourceRef = `<change>/<phase>/<role>/<attempt>`（D3），会话转录联动按此
 * exact-match 反查 agentSessions（source='change'）。
 */
export type FlowRoleLabel = 'executor' | 'evaluator' | 'decision';

/** 转录联动反查键：role × attempt → sourceRef 定式组装产物。 */
export interface RoleSessionRef {
  role: FlowRoleLabel;
  sourceRef: string;
}

/** run 步事件：来自 RunUpdate::Step 流的图 overlay（run-state.ts 推导）。
 * 节点载荷（runStepKind / group / role / status / sessionId / detail 六面）
 * 即 react-flow 侧 `RunStepNodeData` 的字段源（run-step-node.tsx 按本投影
 * 定义 `{ node: RuntimeFlowNode }` 载荷）。 */
export interface RuntimeFlowNode extends FlowNodeBase {
  kind: 'runtime';
  runStepKind: ChangeStepKind;
  group: RunStepGroup;
  role: FlowRoleLabel | null;
  status: ChangeStepStatus;
  sessionId: string | null;
  detail: string | null;
}

export type FlowNode = EvalFlowNode | ActiveFlowNode | InterruptedFlowNode | RuntimeFlowNode;

/** phase 列容器（9 站恒定，未走的站呈现空列） */
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
  /** 回跳边取目标 eval 节点 record.backtrackReason；其余边为 null */
  label: string | null;
}

export interface FlowGraph {
  columns: FlowColumn[];
  /** 事件节点按时间序归并后的执行序排列（列容器经 columns 单列承载） */
  nodes: FlowNode[];
  edges: FlowEdge[];
}

/**
 * 挂载后的过程素材：文档挂列（columnDocs 键 = 列 id `col:<phase>`），
 * 记录挂节点（nodeChecklists / nodeFiles 键 = 事件节点 id）；未命中条目归
 * outsideFiles 图外展示，信息不丢。
 */
export interface FlowMaterials {
  columnDocs: Record<string, ArtifactEnvelope[]>;
  nodeChecklists: Record<string, ArtifactEnvelope[]>;
  nodeFiles: Record<string, FileLogEntry[]>;
  outsideFiles: FileLogEntry[];
}

/** 抽屉选中对象：列头（按 phase）与事件节点（按节点 id）共用同一交互入口 */
export type DrawerSelection =
  | { scope: 'column'; phase: string }
  | { scope: 'node'; nodeId: string };
