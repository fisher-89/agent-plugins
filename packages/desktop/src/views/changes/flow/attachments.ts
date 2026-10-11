import type { ArtifactEnvelope, ChangeDetail } from '../../../types/dto';
import { PIPELINE_PHASES } from './layout';
import type { FlowGraph, FlowMaterials, FlowNode, FlowNodeKind } from './types';

/** 同号多类节点并存时的挂载定位优先级：eval 优先，其次 active */
const NODE_PRECEDENCE: readonly FlowNodeKind[] = ['eval', 'active'];

/**
 * 文档挂列：归属相位自后端清单出线（`detail.artifacts[].phase`——phase 定义清单
 * 为归属权威源，前端字符串 → 列静态映射已退役）。空归属 / pipeline 外相位 →
 * 不挂列（未归属文档不误挂到错误站）。命中返回列 id，否则 null。
 */
function artifactColumn(phase: string | null): string | null {
  if (phase === null || phase === '' || PIPELINE_PHASES.indexOf(phase) < 0) return null;
  return `col:${phase}`;
}

/** eval-checklist payload 最小形状（仅 phase + attempt，供挂载定位；渲染收窄归 renderers 层） */
interface ChecklistRefPayload {
  phase: string;
  attempt: number | null;
}

function isChecklistRef(value: unknown): value is ChecklistRefPayload {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<ChecklistRefPayload>;
  if (typeof candidate.phase !== 'string') return false;
  return candidate.attempt === null || typeof candidate.attempt === 'number';
}

function locateNode(nodes: FlowNode[], phase: string, attempt: number): FlowNode | null {
  for (const kind of NODE_PRECEDENCE) {
    const hit = nodes.find(
      (node) => node.kind === kind && node.phase === phase && node.attempt === attempt,
    );
    if (hit !== undefined) return hit;
  }
  return null;
}

function append<T>(record: Record<string, T[]>, key: string, value: T): void {
  const bucket = record[key];
  if (bucket === undefined) record[key] = [value];
  else bucket.push(value);
}

/**
 * 两类素材按挂载规则归位：文档按清单归属挂列、eval-checklist 按 (phase, attempt)
 * 投影挂节点。
 */
export function mountMaterials(
  graph: FlowGraph,
  detail: ChangeDetail,
  envelopes: ArtifactEnvelope[],
): FlowMaterials {
  const materials: FlowMaterials = {
    columnDocs: {},
    nodeChecklists: {},
  };
  for (let index = 0; index < envelopes.length; index += 1) {
    const envelope = envelopes[index];
    if (envelope.kind === 'markdown-doc' || envelope.kind === 'tasks-progress') {
      // 信封本身无归属：取数层按 detail.artifacts 顺序 1:1 生成信封，按下标配对
      // 取清单归属相位（source 仅作寻址键，归属恒读 phase）
      const descriptor = detail.artifacts[index];
      if (descriptor === undefined) continue;
      const column = artifactColumn(descriptor.phase);
      if (column !== null) append(materials.columnDocs, column, envelope);
    } else if (envelope.kind === 'eval-checklist' && isChecklistRef(envelope.payload)) {
      const node = locateNode(graph.nodes, envelope.payload.phase, envelope.payload.attempt ?? 0);
      if (node !== null) append(materials.nodeChecklists, node.id, envelope);
    }
  }
  return materials;
}
