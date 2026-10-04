import type { ArtifactEnvelope, ChangeDetail } from '../../../types/dto';
import type { FlowGraph, FlowMaterials, FlowNode, FlowNodeKind } from './types';

/** 同号多类节点并存时的挂载定位优先级：eval 优先，其次 active */
const NODE_PRECEDENCE: readonly FlowNodeKind[] = ['eval', 'active'];

/**
 * 文档 → 列静态映射（模块私有，不导出——knip 会把仅测试引用的导出判为未用；
 * 归属规则全部经 mountMaterials 公共输出覆盖测试）。命中返回列 id，表外返回 null。
 */
function docColumn(source: string): string | null {
  if (source === 'proposal.md' || source.startsWith('specs/')) return 'col:proposal';
  if (source === 'design.md' || source === 'tasks.md') return 'col:dev-design';
  if (source.startsWith('test-reports/')) return 'col:test-execution';
  return null;
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
 * 三类素材按挂载规则归位；未命中条目归 outsideFiles，不丢信息。
 * fileLog 为 null（v1 及更早代际）与空数组同型：文件类素材全空输出。
 */
export function mountMaterials(
  graph: FlowGraph,
  detail: ChangeDetail,
  envelopes: ArtifactEnvelope[],
): FlowMaterials {
  const materials: FlowMaterials = {
    columnDocs: {},
    nodeChecklists: {},
    nodeFiles: {},
    outsideFiles: [],
  };
  for (let index = 0; index < envelopes.length; index += 1) {
    const envelope = envelopes[index];
    if (envelope.kind === 'markdown-doc' || envelope.kind === 'tasks-progress') {
      // 信封本身无 source：取数层按 detail.artifacts 顺序 1:1 生成信封，按下标配对取路径
      const source = detail.artifacts[index]?.source;
      if (source === undefined) continue;
      const column = docColumn(source);
      if (column !== null) append(materials.columnDocs, column, envelope);
    } else if (envelope.kind === 'eval-checklist' && isChecklistRef(envelope.payload)) {
      const node = locateNode(graph.nodes, envelope.payload.phase, envelope.payload.attempt ?? 0);
      if (node !== null) append(materials.nodeChecklists, node.id, envelope);
    }
  }
  if (detail.fileLog !== null) {
    for (const entry of detail.fileLog) {
      const node =
        entry.attempt === null ? null : locateNode(graph.nodes, entry.scope, entry.attempt);
      if (node === null) materials.outsideFiles.push(entry);
      else append(materials.nodeFiles, node.id, entry);
    }
  }
  return materials;
}
