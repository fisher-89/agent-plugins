import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { ArtifactEnvelope, ChangeDetail, Inventory } from '../../types/dto';
import { mountMaterials } from './flow/attachments';
import { ChangeFlowGraph } from './flow/change-flow-graph';
import { DetailDrawer } from './flow/detail-drawer';
import { FileLogTable } from './flow/file-log-table';
import { buildFlowGraph } from './flow/graph';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './flow/types';
import { useChangeDetail } from './hooks/use-change-detail';
import { ArtifactTabs } from './renderers/artifact-tabs';

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

/** 产物区：多文档经 ArtifactTabs 切换（共享组件，语义见 artifact-tabs.tsx）；
 * 空清单以本节文案占位。 */
function DetailSectionArtifacts({ artifacts }: { artifacts: ArtifactEnvelope[] }) {
  if (artifacts.length === 0) {
    return (
      <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
        <h2 className="m-0 mb-2.5 text-[15px]">
          产物 <span className="text-muted-foreground">(0)</span>
        </h2>
        <div className="text-muted-foreground">（未发现可读产物）</div>
      </section>
    );
  }
  return (
    <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
      <h2 className="m-0 mb-2.5 text-[15px]">
        产物 <span className="text-muted-foreground">({artifacts.length})</span>
      </h2>
      <ArtifactTabs artifacts={artifacts} />
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
 * 根切换抑制（原 ChangeView 语义迁入）：workspace 根变更（select / 移除当前根 /
 * 添加新根）且详情仍带旧选中时，过渡轮以 null 抑制取数（防「新根 + 旧名」误发
 * get_change_detail），effect 中 replace 导航回 /changes（URL 无 :name 段）；
 * 落点后详情页随路由卸载，抑制位无须显式清位（清位早于落点提交反而会留出
 * 「新根 + 旧名」中间提交，重新武装误发）。返回参与取数的 change 名。
 */
function useRootSwitchSuppress(root: string | null, selected: string | null): string | null {
  const navigate = useNavigate();
  const [prevRoot, setPrevRoot] = useState(root);
  const [resetPending, setResetPending] = useState(false);

  // 渲染期调整（沿用既有模式）：根切换且带旧选中 → 置待导航标记
  if (prevRoot !== root) {
    setPrevRoot(root);
    if (selected !== null) setResetPending(true);
  }

  useEffect(() => {
    if (resetPending) {
      void navigate('/changes', { replace: true }); // workspace 切换落清单，URL 无 :name 段
    }
  }, [resetPending, navigate]);

  return resetPending ? null : selected;
}

/**
 * change 详情视图：详情页自取数（useChangeDetail 按 (root, URL name) 调
 * get_change_detail，与清单页互不依赖；根切换抑制见 useRootSwitchSuppress）
 * + Header + attempt 级流程图 + workflow 独立面板 + 产物区 + 抽屉。三代际降级
 * （design）：v0（pipeline 空）空图占位 + 产物区；v1（fileLog null）图正常
 * 绘制、无 workflow 面板、抽屉文件表节降级；v2 完整图。
 */
export function ChangeDetailView({ root }: { root: string | null }) {
  const { name } = useParams<'name'>();
  const selected = useRootSwitchSuppress(root, name ?? null);
  const { detail, artifacts, loading, error, refresh } = useChangeDetail(root, selected);
  const navigate = useNavigate();
  const backToList = useCallback(() => navigate('/changes'), [navigate]); // 显式返回，不用 navigate(-1)

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
    return <DetailFallback message={`详情加载失败：${error}`} error onBack={backToList} />;
  }
  if (loading && detail === null) {
    return <DetailFallback message="加载中…" onBack={backToList} />;
  }
  if (detail === null) {
    return <DetailFallback message="未找到该 change。" onBack={backToList} />;
  }
  return (
    <div>
      <DetailHeader detail={detail} loading={loading} onBack={backToList} refresh={refresh} />
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
