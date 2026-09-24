/**
 * 核心转换纯函数：ChangeDetail 聚合 → 流程图模型（buildFlowGraph）。
 *
 * 三步（O(n)）：
 * 1. 三分类事件收集：eval 按站序 + 站内 attempts 序（后端已按 attempt 稳定排序，
 *    追加序即事实执行序）、interrupted[]、activePhase（phase 不在 9 站内的事件跳过）；
 * 2. 时间序归并：eval 追加序为主链不重排；interrupted / active 按 startAt
 *    （Date.parse 毫秒，解析失败或缺失视同 null）插入骨干中第一个锚点严格晚于它的
 *    eval 事件之前；骨干锚点为 null 的 eval 不作插入参考；无插入点者按列表序
 *    （先 interrupted[] 后 active）追加链尾；
 * 3. 链式边推导：归并序列头尾相连，每个非首事件恰一条入边，kind 由两端列索引差
 *    符号派生（>0 forward / =0 retry / <0 backtrack），回跳边 label 取目标
 *    eval 节点 record.backtrackReason。
 */
import type { ChangeDetail, PhaseEntry } from '../../../types/dto';
import { PIPELINE_PHASES } from './layout';
import type {
  ActiveFlowNode,
  EvalFlowNode,
  FlowColumn,
  FlowEdge,
  FlowEdgeKind,
  FlowGraph,
  FlowNode,
  InterruptedFlowNode,
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
  return evals;
}

function collectInterrupted(detail: ChangeDetail): InterruptedFlowNode[] {
  const nodes: InterruptedFlowNode[] = [];
  for (const entry of detail.interrupted) {
    const colIndex = colIndexOf(entry.phase);
    if (colIndex < 0) continue;
    nodes.push({
      id: `interrupted:${entry.phase}:${entry.attempt}`,
      kind: 'interrupted',
      phase: entry.phase,
      attempt: entry.attempt,
      colIndex,
      order: 0,
      parentId: `col:${entry.phase}`,
      startAt: entry.startAt,
      endAt: entry.endAt,
    });
  }
  return nodes;
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

/** 骨干 eval 追加序 + interrupted / active 按 startAt 插入（先 interrupted 后 active 追加链尾） */
function mergeByStartAt(
  backbone: EvalFlowNode[],
  merging: (ActiveFlowNode | InterruptedFlowNode)[],
): FlowNode[] {
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

function deriveEdges(sequence: FlowNode[]): FlowEdge[] {
  const edges: FlowEdge[] = [];
  for (let index = 1; index < sequence.length; index += 1) {
    const source = sequence[index - 1];
    const target = sequence[index];
    const delta = target.colIndex - source.colIndex;
    const kind: FlowEdgeKind = delta > 0 ? 'forward' : delta === 0 ? 'retry' : 'backtrack';
    edges.push({
      id: `edge:${source.id}->${target.id}`,
      source: source.id,
      target: target.id,
      kind,
      label: kind === 'backtrack' && target.kind === 'eval' ? target.record.backtrackReason : null,
    });
  }
  return edges;
}

/** 详情聚合 → 流程图模型；`pipeline` 为空（v0 早期代际）返回空图（无列无节点） */
export function buildFlowGraph(detail: ChangeDetail): FlowGraph {
  if (detail.pipeline.length === 0) {
    return { columns: [], nodes: [], edges: [] };
  }
  const columns: FlowColumn[] = PIPELINE_PHASES.map((phase, colIndex) => ({
    id: `col:${phase}`,
    phase,
    colIndex,
  }));
  const backbone = collectEvals(detail.pipeline);
  // 归并列表序：先 interrupted[] 后 active（两者 startAt 均无插入点时的链尾次序）
  const sequence = mergeByStartAt(backbone, [
    ...collectInterrupted(detail),
    ...collectActive(detail),
  ]);
  assignOrders(sequence);
  return { columns, nodes: sequence, edges: deriveEdges(sequence) };
}
