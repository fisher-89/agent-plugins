import { useMemo, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { ArtifactEnvelope, ChangeDetail, Inventory } from '../../types/dto';
import { mountMaterials } from './flow/attachments';
import { ChangeFlowGraph } from './flow/change-flow-graph';
import { DetailDrawer } from './flow/detail-drawer';
import { FileLogTable } from './flow/file-log-table';
import { buildFlowGraph } from './flow/graph';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './flow/types';
import type { ChangeDetailState } from './hooks/use-change-detail';
import { ArtifactView } from './renderers/artifact-view';

// Tailwind 无法静态识别模板串类名：`badge-in${inventory}` 收敛为显式 variant 映射（spec 硬性要求）
const INVENTORY_VARIANT: Record<Inventory, 'inv0' | 'inv1' | 'inv2'> = {
  v0: 'inv0',
  v1: 'inv1',
  v2: 'inv2',
};

// detail 为 null（降级页分支）时 hooks 仍需无条件产出图数据的空底座
const EMPTY_GRAPH: FlowGraph = { columns: [], nodes: [], edges: [] };
const EMPTY_MATERIALS: FlowMaterials = {
  columnDocs: {},
  nodeChecklists: {},
  nodeFiles: {},
  outsideFiles: [],
};

function formatTime(value: string | null): string {
  if (value === null) return '—';
  return value;
}

function DetailHeader({
  detail,
  loading,
  onBack,
  refresh,
}: {
  detail: ChangeDetail;
  loading: boolean;
  onBack: () => void;
  refresh: () => void;
}) {
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2.5" data-testid="detail-header">
      <Button onClick={onBack}>← 返回列表</Button>
      <Button onClick={refresh} disabled={loading}>
        刷新详情
      </Button>
      <h2 className="m-0 break-all text-[17px]">{detail.name}</h2>
      <Badge variant={INVENTORY_VARIANT[detail.inventory]}>{detail.inventory}</Badge>
      <span className="text-muted-foreground">
        {detail.source === 'archive' ? '已归档' : '进行中'}
      </span>
      {detail.created !== null && <span className="text-muted-foreground">{detail.created}</span>}
      {detail.activePhase !== null && (
        <Badge variant="active">
          运行中 · {detail.activePhase.phase} · attempt {detail.activePhase.attempt}
          {detail.activePhase.startAt !== null && ` · ${formatTime(detail.activePhase.startAt)}`}
        </Badge>
      )}
    </div>
  );
}

/** 流程图区：v0（pipeline 空，无 workflow.json）空图占位，其余挂载 ChangeFlowGraph */
function FlowSection({
  detail,
  graph,
  materials,
  onSelect,
}: {
  detail: ChangeDetail;
  graph: FlowGraph;
  materials: FlowMaterials;
  onSelect: (selection: DrawerSelection) => void;
}) {
  if (detail.pipeline.length === 0) {
    return (
      <section
        className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
        data-testid="flow-empty"
      >
        <h2 className="m-0 mb-2.5 text-[15px]">流程图</h2>
        <div className="text-muted-foreground">（v0 早期代际：无 workflow.json，仅文档形态）</div>
      </section>
    );
  }
  return (
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">流程图</h2>
      <ChangeFlowGraph graph={graph} materials={materials} onSelect={onSelect} />
    </section>
  );
}

function DetailSectionArtifacts({ artifacts }: { artifacts: ArtifactEnvelope[] }) {
  return (
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">
        产物 <span className="text-muted-foreground">({artifacts.length})</span>
      </h2>
      {artifacts.length === 0 ? (
        <div className="text-muted-foreground">（未发现可读产物）</div>
      ) : (
        artifacts.map((envelope, index) => (
          <ArtifactView key={`${envelope.kind}-${envelope.title}-${index}`} envelope={envelope} />
        ))
      )}
    </section>
  );
}

/** 详情降级页：错误 / 加载中 / 未找到共用，error 控制提示样式。 */
function DetailFallback({
  message,
  error = false,
  onBack,
}: {
  message: string;
  error?: boolean;
  onBack: () => void;
}) {
  return (
    <div>
      <Button onClick={onBack}>← 返回列表</Button>
      {error ? (
        <div
          className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="error-note"
        >
          {message}
        </div>
      ) : (
        <div className="text-muted-foreground" data-testid="detail-note">
          {message}
        </div>
      )}
    </div>
  );
}

/** workflow.json 损坏警示条（unparsable 时呈现） */
function UnparsableNote() {
  return (
    <div
      className="my-2 rounded-md bg-warn-bg px-2.5 py-1.5 text-[13px] text-warn"
      data-testid="warn-note"
    >
      workflow.json 无法解析（可能已损坏），以下仅展示文件系统层信息与产物。
    </div>
  );
}

/** workflow 独立面板：scope='workflow' 与未命中节点的 file_log 条目（图外完整展示） */
function WorkflowPanel({ entries }: { entries: ChangeDetail['fileLog'] }) {
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="workflow-panel"
    >
      <h2 className="m-0 mb-2.5 text-[15px]">workflow 文件清单 (file_log)</h2>
      <FileLogTable entries={entries ?? []} />
    </section>
  );
}

/**
 * change 详情视图：Header + attempt 级流程图 + workflow 独立面板 + 产物区 + 抽屉。
 * 三代际降级（design）：v0（pipeline 空）空图占位 + 产物区；v1（fileLog null）图
 * 正常绘制、无 workflow 面板、抽屉文件表节降级；v2 完整图。取数仍仅由显式 refresh
 * 触发（useChangeDetail 零改动），图数据仅随 state.detail 重算。
 */
export function ChangeDetailView({
  state,
  onBack,
}: {
  state: ChangeDetailState;
  onBack: () => void;
}) {
  const { detail, artifacts, loading, error, refresh } = state;
  const [selection, setSelection] = useState<DrawerSelection | null>(null);
  const graph = useMemo<FlowGraph>(
    () => (detail === null ? EMPTY_GRAPH : buildFlowGraph(detail)),
    [detail],
  );
  const materials = useMemo<FlowMaterials>(
    () => (detail === null ? EMPTY_MATERIALS : mountMaterials(graph, detail, artifacts)),
    [detail, graph, artifacts],
  );
  if (error !== null) {
    return <DetailFallback message={`详情加载失败：${error}`} error onBack={onBack} />;
  }
  if (loading && detail === null) {
    return <DetailFallback message="加载中…" onBack={onBack} />;
  }
  if (detail === null) {
    return <DetailFallback message="未找到该 change。" onBack={onBack} />;
  }
  return (
    <div>
      <DetailHeader detail={detail} loading={loading} onBack={onBack} refresh={refresh} />
      {detail.unparsable && <UnparsableNote />}
      <FlowSection detail={detail} graph={graph} materials={materials} onSelect={setSelection} />
      {detail.fileLog !== null && <WorkflowPanel entries={materials.outsideFiles} />}
      <DetailSectionArtifacts artifacts={artifacts} />
      <DetailDrawer
        selection={selection}
        graph={graph}
        materials={materials}
        hasFileLog={detail.fileLog !== null}
        onClose={() => setSelection(null)}
      />
    </div>
  );
}
