import type { ArtifactEnvelope, ChangeDetail } from '../../../types/dto';
import type { FlowMaterials } from './types';

/**
 * 文档挂列：归属相位自后端清单出线（`detail.artifacts[].phase`——phase 定义清单
 * 为归属权威源，前端字符串 → 列静态映射已退役）。归属校验以该 change
 * `detail.pipeline` 的相位集为准（列序出线单源：requirement 9 列 / test-only
 * 6 列），空归属 / pipeline 外相位 → 不挂列（未归属文档不误挂到错误站）。命中
 * 返回列 id，否则 null。
 */
function artifactColumn(phase: string | null, pipelinePhases: Set<string>): string | null {
  if (phase === null || phase === '' || !pipelinePhases.has(phase)) return null;
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

function append<T>(record: Record<string, T[]>, key: string, value: T): void {
  const bucket = record[key];
  if (bucket === undefined) record[key] = [value];
  else bucket.push(value);
}

/**
 * 素材挂载单层：文档按清单归属、eval-checklist 按 payload 自带 `phase` 归属，**均
 * 挂 phase 列头**（`columnDocs` 键 = 列 id；列头抽屉经 `ArtifactTabs` 呈现该站
 * 全部过程素材，checklist 全套元数据由 `EvalChecklistRenderer` 渲染）。挂节点规则
 * 随 eval / active 相位节点退役一并删除（`nodeChecklists` 退役）。
 */
export function mountMaterials(detail: ChangeDetail, envelopes: ArtifactEnvelope[]): FlowMaterials {
  const materials: FlowMaterials = { columnDocs: {} };
  // 归属校验的相位集 = `detail.pipeline` 相位集（列序出线单源；零静态相位序常量）
  const pipelinePhases = new Set(detail.pipeline.map((entry) => entry.phase));
  for (let index = 0; index < envelopes.length; index += 1) {
    const envelope = envelopes[index];
    if (envelope.kind === 'markdown-doc' || envelope.kind === 'tasks-progress') {
      // 信封本身无归属：取数层按 detail.artifacts 顺序 1:1 生成信封，按下标配对
      // 取清单归属相位（source 仅作寻址键，归属恒读 phase）
      const descriptor = detail.artifacts[index];
      if (descriptor === undefined) continue;
      const column = artifactColumn(descriptor.phase, pipelinePhases);
      if (column !== null) append(materials.columnDocs, column, envelope);
    } else if (envelope.kind === 'eval-checklist' && isChecklistRef(envelope.payload)) {
      // payload 自带 phase（pipeline 外相位不挂列——归属校验与文档同式）
      const column = artifactColumn(envelope.payload.phase, pipelinePhases);
      if (column !== null) append(materials.columnDocs, column, envelope);
    }
  }
  return materials;
}
