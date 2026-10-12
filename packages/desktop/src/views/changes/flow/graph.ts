import type { ChangeDetail, PhaseEntry } from '../../../types/dto';
import { runNodeKey, type UnifiedRunEdge } from './run-state';
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

/** 后端边表 → 图边（端点按「run id + 节点稳定身份键」寻址——边表端点身份串
 * 无 run 段，跨 run 同名身份串以边行归属 run 段前拼比对，重启重跑同 attempt
 * 的跨轮边不锚错他轮节点；悬空端点降级跳过不渲染——数据缺口不炸图）。列差
 * Δ 派生已退役。 */
function tableEdges(
  edges: readonly UnifiedRunEdge[],
  nodes: FlowNode[],
  pipeline: PhaseEntry[],
): FlowEdge[] {
  const byKey = new Map<string, RuntimeFlowNode>();
  for (const node of nodes) {
    if (node.kind === 'runtime') byKey.set(runNodeKey(node), node);
  }
  const flowEdges: FlowEdge[] = [];
  for (let index = 0; index < edges.length; index += 1) {
    const edge = edges[index];
    const source = byKey.get(`${edge.runId}:${edge.fromNodeKey}`);
    const target = byKey.get(`${edge.runId}:${edge.toNodeKey}`);
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

/** 详情聚合 → 流程图模型 */
export function buildFlowGraph(
  detail: ChangeDetail,
  runNodes?: RuntimeFlowNode[],
  runEdges?: readonly UnifiedRunEdge[],
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
