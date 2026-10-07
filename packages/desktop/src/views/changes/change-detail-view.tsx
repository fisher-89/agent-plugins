import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { AgentEvent, ArtifactEnvelope, ChangeDetail } from '../../types/dto';
import { mountMaterials } from './flow/attachments';
import { ChangeFlowGraph } from './flow/change-flow-graph';
import { DetailDrawer } from './flow/detail-drawer';
import { buildFlowGraph } from './flow/graph';
import { RunControlPanel } from './flow/run-control-panel';
import { runStepNodes } from './flow/run-state';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './flow/types';
import { useChangeDetail } from './hooks/use-change-detail';
import { useChangeFlowRun } from './hooks/use-change-flow-run';
import { ArtifactTabs } from './renderers/artifact-tabs';

// detail 为 null（降级页分支）时 hooks 仍需无条件产出图数据的空底座
const EMPTY_GRAPH: FlowGraph = { columns: [], nodes: [], edges: [] };
const EMPTY_MATERIALS: FlowMaterials = {
  columnDocs: {},
  nodeChecklists: {},
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
    <div className="mb-3" data-testid="detail-header">
      <div className="flex flex-wrap items-center gap-2.5">
        <Button onClick={onBack}>← 返回列表</Button>
        <Button onClick={refresh} disabled={loading}>
          刷新详情
        </Button>
        <h2 className="m-0 break-all text-[17px]">{detail.name}</h2>
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
      {detail.worktree !== null && (
        <div className="mt-1 break-all text-xs text-muted-foreground" data-testid="detail-worktree">
          worktree：{detail.worktree}
        </div>
      )}
    </div>
  );
}

/** 流程图区：建档判别两态分流（status 在场 = 建档挂载 ChangeFlowGraph；缺席
 * = 文档形态空图占位，产物区照常不挤掉） */
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
  if (detail.status === null) {
    return (
      <section
        className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
        data-testid="flow-empty"
      >
        <h2 className="m-0 mb-2.5 text-[15px]">流程图</h2>
        <div className="text-muted-foreground">（文档形态：未建档，仅产物清单）</div>
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

/** run 视图副作用：终态触发一次显式 refresh（图回落 ChangeDetail 派生规则；
 * 状态迁移沿「非终局 → 终局」判定，重挂不重复触发）+ 实时事件展平（抽屉
 * 过滤输入面）。 */
function useRunViewEffects(
  run: ReturnType<typeof useChangeFlowRun>,
  refresh: () => void,
): Array<{ sessionId: string; event: AgentEvent }> {
  const prevStatus = useRef<string | null>(null);
  const status = run.state?.status ?? null;
  useEffect(() => {
    const wasTerminal = prevStatus.current !== null && isTerminalStatus(prevStatus.current);
    prevStatus.current = status;
    if (!wasTerminal && status !== null && isTerminalStatus(status)) {
      refresh();
    }
  }, [status, refresh]);
  return useMemo(() => {
    if (run.state === null) return [];
    return Object.entries(run.state.liveEvents).flatMap(([sessionId, events]) =>
      events.map((event) => ({ sessionId, event })),
    );
  }, [run.state]);
}

function isTerminalStatus(status: string): boolean {
  return status === 'completed' || status === 'stopped' || status === 'failed';
}

/**
 * change 详情视图：详情页自取数（useChangeDetail 按 (root, URL name) 调
 * get_change_detail，与清单页互不依赖；根切换抑制见 useRootSwitchSuppress）
 * + Header + 运行控制面板 + attempt 级流程图（运行步 overlay 并入）+
 * 产物区 + 抽屉（WorkerAgent 运行节点与 eval 节点的会话转录联动）。
 * 建档两态分流（design）：建档（status 在场）完整状态面；文档形态（status
 * 缺席，存量 CLI change）空图占位 + 产物区。
 *
 * run 生命周期：useChangeFlowRun 承载 invoke 与订阅；run 终态时触发一次
 * 显式 refresh（图回落派生规则——运行外显式刷新仍是唯一全量更新途径）。
 */
export function ChangeDetailView({ root }: { root: string | null }) {
  const { name } = useParams<'name'>();
  const selected = useRootSwitchSuppress(root, name ?? null);
  const { detail, artifacts, loading, error, refresh } = useChangeDetail(root, selected);
  const run = useChangeFlowRun({ root, change: selected });
  const navigate = useNavigate();
  const backToList = useCallback(() => navigate('/changes'), [navigate]); // 显式返回，不用 navigate(-1)

  const [selection, setSelection] = useState<DrawerSelection | null>(null);
  const runNodes = useMemo(() => (run.state === null ? [] : runStepNodes(run.state)), [run.state]);
  const graph = useMemo<FlowGraph>(
    () => (detail === null ? EMPTY_GRAPH : buildFlowGraph(detail, runNodes)),
    [detail, runNodes],
  );
  const materials = useMemo<FlowMaterials>(
    () => (detail === null ? EMPTY_MATERIALS : mountMaterials(graph, detail, artifacts)),
    [detail, graph, artifacts],
  );
  const liveEvents = useRunViewEffects(run, refresh);

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
      {selected !== null && <RunControlPanel change={selected} run={run} />}
      <FlowSection detail={detail} graph={graph} materials={materials} onSelect={setSelection} />
      <DetailSectionArtifacts artifacts={artifacts} />
      <DetailDrawer
        selection={selection}
        graph={graph}
        materials={materials}
        root={root}
        change={detail.name}
        liveEvents={liveEvents}
        onClose={() => setSelection(null)}
      />
    </div>
  );
}
