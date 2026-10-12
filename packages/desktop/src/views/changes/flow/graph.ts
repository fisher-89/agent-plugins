import type { ChangeDetail, ChangeRunEdgeRecord, PhaseEntry } from '../../../types/dto';
import { nodeIdentityKey } from './run-state';
import type { FlowColumn, FlowEdge, FlowGraph, FlowNode, RuntimeFlowNode } from './types';

/** 列内归并序：同列节点按归并后相对次序 0 起编号 */
function assignOrders(sequence: FlowNode[]): void {
  const counters = new Map<string, number>();
  for (const node of sequence) {
    const order = counters.get(node.phase) ?? 0;
    node.order = order;
    counters.set(node.phase, order + 1);
  }
}

/** 回跳边标签：发起相位最近一次条目的 `backtrackReason`（回跳落账记因随发起相位
 * 条目在库；无边标签来源则 null）。 */
function backtrackReason(pipeline: PhaseEntry[], phase: string): string | null {
  const station = pipeline.find((candidate) => candidate.phase === phase);
  if (station === undefined) return null;
  for (let index = station.attempts.length - 1; index >= 0; index -= 1) {
    const reason = station.attempts[index].backtrackReason;
    if (reason !== null && reason !== '') return reason;
  }
  return null;
}

/** 后端边表 → 图边（端点按节点稳定身份键寻址；悬空端点降级跳过不渲染——数据
 * 缺口不炸图）。列差 Δ 派生已退役。 */
function tableEdges(
  edges: readonly ChangeRunEdgeRecord[],
  nodes: FlowNode[],
  pipeline: PhaseEntry[],
): FlowEdge[] {
  const byKey = new Map<string, RuntimeFlowNode>();
  for (const node of nodes) {
    if (node.kind === 'runtime') byKey.set(nodeIdentityKey(node), node);
  }
  const flowEdges: FlowEdge[] = [];
  for (let index = 0; index < edges.length; index += 1) {
    const edge = edges[index];
    const source = byKey.get(edge.fromNodeKey);
    const target = byKey.get(edge.toNodeKey);
    if (source === undefined || target === undefined) continue;
    flowEdges.push({
      id: `edge:${index}:${source.id}->${target.id}`,
      source: source.id,
      target: target.id,
      kind: edge.edgeKind,
      label: edge.edgeKind === 'backtrack' ? backtrackReason(pipeline, source.phase) : null,
    });
  }
  return flowEdges;
}

/** 详情聚合 → 流程图模型；`pipeline` 为空（v0 早期代际）返回空图（无列无节点）。
 * 节点模型为**单分类：仅 runtime step 节点**（eval / active 相位节点整体退役，
 * `sequence = [...runNodes]`；运行中由 running 态 runtime 节点承载）。`runNodes` /
 * `runEdges` 为运行执行图 overlay（统一视图派生——`runStepNodes(unifiedRunNodes(
 * detail.runs, activeRun?.nodes))` + `unifiedRunEdges(detail.runs, activeRun?.edges)`
 * 单源拼装：库读史 ∪ 在飞活面同一转换函数，图常驻渲染零回落；缺省 / 空参即无
 * 运行态）。`pipeline` 仍供 `tableEdges` 的 backtrack 边 label（回跳记因）与列序
 * （`columns` 由 `detail.pipeline` 出线序派生，前端零静态相位序常量）。 */
export function buildFlowGraph(
  detail: ChangeDetail,
  runNodes?: RuntimeFlowNode[],
  runEdges?: readonly ChangeRunEdgeRecord[],
): FlowGraph {
  if (detail.pipeline.length === 0) {
    return { columns: [], nodes: [], edges: [] };
  }
  // 列序单源 = `detail.pipeline`（后端相位表出线序：requirement 9 列 /
  // test-only 6 列；colIndex = 数组下标，列 id = `col:<phase>`）
  const columns: FlowColumn[] = detail.pipeline.map((entry, colIndex) => ({
    id: `col:${entry.phase}`,
    phase: entry.phase,
    colIndex,
  }));
  const sequence: FlowNode[] = [...(runNodes ?? [])];
  assignOrders(sequence);
  return {
    columns,
    nodes: sequence,
    edges: tableEdges(runEdges ?? [], sequence, detail.pipeline),
  };
}
