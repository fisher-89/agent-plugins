import type { ChangeDetail, ChangeRunEdgeRecord, PhaseEntry } from '../../../types/dto';
import { PIPELINE_PHASES } from './layout';
import { nodeIdentityKey } from './run-state';
import type {
  ActiveFlowNode,
  EvalFlowNode,
  FlowColumn,
  FlowEdge,
  FlowGraph,
  FlowNode,
  RuntimeFlowNode,
} from './types';

/** 锚点时间（毫秒）；null 或 Date.parse 产出 NaN（非法时间串）均视同 null，不参与比较 */
function anchorMs(value: string | null): number | null {
  if (value === null) return null;
  const ms = Date.parse(value);
  return Number.isNaN(ms) ? null : ms;
}

function colIndexOf(phase: string): number {
  return PIPELINE_PHASES.indexOf(phase);
}

function collectEvals(pipeline: PhaseEntry[]): EvalFlowNode[] {
  const evals: EvalFlowNode[] = [];
  const usedIds = new Set<string>();
  for (const station of pipeline) {
    const colIndex = colIndexOf(station.phase);
    if (colIndex < 0) continue;
    for (const record of station.attempts) {
      const attempt = record.attempt ?? 0;
      // 同站缺号兜底撞车（同 phase 多条 attempt null）时加序号后缀，避免 react-flow 重复 id
      let id = `eval:${station.phase}:${attempt}`;
      if (usedIds.has(id)) id = `${id}#${evals.length}`;
      usedIds.add(id);
      evals.push({
        id,
        kind: 'eval',
        phase: station.phase,
        attempt,
        colIndex,
        order: 0,
        parentId: `col:${station.phase}`,
        record,
      });
    }
  }
  return evals.sort(
    (a, b) => (anchorMs(a.record.startAt) ?? 0) - (anchorMs(b.record.startAt) ?? 0),
  );
}

function collectActive(detail: ChangeDetail): ActiveFlowNode[] {
  const active = detail.activePhase;
  if (active === null) return [];
  const colIndex = colIndexOf(active.phase);
  if (colIndex < 0) return [];
  return [
    {
      id: `active:${active.phase}:${active.attempt}`,
      kind: 'active',
      phase: active.phase,
      attempt: active.attempt,
      colIndex,
      order: 0,
      parentId: `col:${active.phase}`,
      startAt: active.startAt,
    },
  ];
}

/** 骨干 eval 追加序 + active 按 startAt 插入（无插入点者追加链尾） */
function mergeByStartAt(backbone: EvalFlowNode[], merging: ActiveFlowNode[]): FlowNode[] {
  const merged: FlowNode[] = [...backbone];
  for (const event of merging) {
    const anchor = anchorMs(event.startAt);
    let insertAt = merged.length;
    if (anchor !== null) {
      for (let index = 0; index < merged.length; index += 1) {
        const candidate = merged[index];
        if (candidate.kind !== 'eval') continue; // 仅骨干 eval 作插入参考
        const other = anchorMs(candidate.record.startAt);
        if (other !== null && other > anchor) {
          insertAt = index;
          break;
        }
      }
    }
    merged.splice(insertAt, 0, event);
  }
  return merged;
}

/** 列内归并序：同列事件按归并后相对次序 0 起编号 */
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
 * `runNodes` / `runEdges` 为运行执行图 overlay（统一视图派生——`runStepNodes(
 * unifiedRunNodes(detail.runs, activeRun?.nodes))` + `unifiedRunEdges(detail.runs,
 * activeRun?.edges)` 单源拼装：库读史 ∪ 在飞活面同一转换函数，图常驻渲染零回落；
 * 缺省 / 空参即无运行态）。节点按给定序恒追加归并链尾；边恒读后端边表。 */
export function buildFlowGraph(
  detail: ChangeDetail,
  runNodes?: RuntimeFlowNode[],
  runEdges?: readonly ChangeRunEdgeRecord[],
): FlowGraph {
  if (detail.pipeline.length === 0) {
    return { columns: [], nodes: [], edges: [] };
  }
  const columns: FlowColumn[] = PIPELINE_PHASES.map((phase, colIndex) => ({
    id: `col:${phase}`,
    phase,
    colIndex,
  }));
  const backbone = collectEvals(detail.pipeline);
  const sequence = mergeByStartAt(backbone, collectActive(detail));
  for (const runNode of runNodes ?? []) {
    sequence.push(runNode);
  }
  assignOrders(sequence);
  return {
    columns,
    nodes: sequence,
    edges: tableEdges(runEdges ?? [], sequence, detail.pipeline),
  };
}
