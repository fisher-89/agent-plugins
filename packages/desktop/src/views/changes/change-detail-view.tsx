import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type {
  ActiveRunView,
  ArtifactEnvelope,
  ChangeDetail,
  ChangeRunEntry,
} from '../../types/dto';
import type { RunNotice } from '../../types/generated/bindings';
import { mountMaterials } from './flow/attachments';
import { ChangeFlowGraph } from './flow/change-flow-graph';
import { DetailDrawer } from './flow/detail-drawer';
import { buildFlowGraph } from './flow/graph';
import { RunControlPanel } from './flow/run-control-panel';
import { runStepNodes, unifiedRunEdges, unifiedRunNodes } from './flow/run-state';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './flow/types';
import { useChangeDetail } from './hooks/use-change-detail';
import { useChangeFlowRun } from './hooks/use-change-flow-run';

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
        <h2 className="m-0 break-all text-[17px]">{detail.title}</h2>
        <span className="text-muted-foreground">
          {detail.source === 'archive' ? '已归档' : '进行中'}
        </span>
        {detail.created !== null && <span className="text-muted-foreground">{detail.created}</span>}
        {detail.activePhase !== null && (
          <Badge variant="secondary">
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

/** 流程图区单态呈现（文档形态空图占位分支整体退役——未知 id 走未找到降级，
 * 详情可达者恒为建档 change，流程图常驻挂载） */
function FlowSection({
  graph,
  materials,
  onSelect,
}: {
  graph: FlowGraph;
  materials: FlowMaterials;
  onSelect: (selection: DrawerSelection) => void;
}): React.JSX.Element {
  return (
    <section className="h-full mb-4 rounded-lg border border-border bg-card px-4 py-3.5 flex flex-col">
      <h2 className="m-0 mb-2.5 text-[15px]">流程图</h2>
      <ChangeFlowGraph graph={graph} materials={materials} onSelect={onSelect} />
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
 * 添加新根）且详情仍带旧选中时，过渡轮以 null 抑制取数（防「新根 + 旧 id」误发
 * get_change_detail），effect 中 replace 导航回 /changes（URL 无 :id 段）；
 * 落点后详情页随路由卸载，抑制位无须显式清位（清位早于落点提交反而会留出
 * 「新根 + 旧 id」中间提交，重新武装误发）。返回参与取数的 change id。
 */
function useRootSwitchSuppress(root: string | null, id: string | null): string | null {
  const navigate = useNavigate();
  const [prevRoot, setPrevRoot] = useState(root);
  const [resetPending, setResetPending] = useState(false);

  // 渲染期调整（沿用既有模式）：根切换且带旧选中 → 置待导航标记
  if (prevRoot !== root) {
    setPrevRoot(root);
    if (id !== null) setResetPending(true);
  }

  useEffect(() => {
    if (resetPending) {
      void navigate('/changes', { replace: true }); // workspace 切换落清单，URL 无 :id 段
    }
  }, [resetPending, navigate]);

  return resetPending ? null : id;
}

/** 流程图 + 产物区 + 抽屉（selection 联动态自持：图节点点击开抽屉，开关不出
 * 本组件——卸载即复位，与降级页分流语义一致；页内单一身份源 = URL id）。 */
function DetailContent({
  root,
  id,
  name,
  graph,
  materials,
  transcriptRefreshKey,
}: {
  root: string | null;
  id: string;
  /** change name（stdout 面板磁盘面寻址键——与 record 同口径，非 id） */
  name: string;
  graph: FlowGraph;
  materials: FlowMaterials;
  transcriptRefreshKey: number;
}): React.JSX.Element {
  const [selection, setSelection] = useState<DrawerSelection | null>(null);
  return (
    <>
      <FlowSection graph={graph} materials={materials} onSelect={setSelection} />
      <DetailDrawer
        selection={selection}
        graph={graph}
        materials={materials}
        root={root}
        changeId={id}
        name={name}
        transcriptRefreshKey={transcriptRefreshKey}
        onClose={() => setSelection(null)}
      />
    </>
  );
}

interface DetailLoadedProps {
  detail: ChangeDetail;
  /** 在飞 run 活面（统一视图 activeRun） */
  activeRun: ActiveRunView | null;
  root: string | null;
  /** 页内单一身份源（URL id；name 恒展示面取自 detail.name） */
  id: string | null;
  artifacts: ArtifactEnvelope[];
  run: ReturnType<typeof useChangeFlowRun>;
  header: React.JSX.Element;
  transcriptRefreshKey: number;
}

/** 页内动作面板组（运行控制面承载发起 / 停止 / 停等确认 / ask 应答——归档确认
 * 对话由 stop-before-archive 停等面承载；显名收敛：命令面取 URL id、展示面取
 * `detail.name`）。 */
function DetailActionPanels({
  detail,
  id,
  activeRun,
  lastRun,
  run,
}: {
  detail: ChangeDetail;
  id: string | null;
  activeRun: ActiveRunView | null;
  lastRun: ChangeRunEntry | null;
  run: ReturnType<typeof useChangeFlowRun>;
}): React.JSX.Element {
  return (
    <>
      {id !== null && (
        <RunControlPanel
          changeId={id}
          name={detail.name}
          activeRun={activeRun}
          lastRun={lastRun}
          actions={run}
        />
      )}
    </>
  );
}

/** 执行图数据源（拼缝单源化：步节点 = runs[].nodes ∪ activeRun.nodes、边 =
 * runs[].edges ∪ activeRun.edges 同一转换函数——图常驻渲染，回落分支退场）。 */
function useFlowGraphData(
  detail: ChangeDetail,
  activeRun: ActiveRunView | null,
  artifacts: ArtifactEnvelope[],
): { graph: FlowGraph; materials: FlowMaterials; lastRun: ChangeRunEntry | null } {
  // 列序单源 = `detail.pipeline` 出线序（requirement 9 列 / test-only 6 列）
  const phaseOrder = useMemo(() => detail.pipeline.map((entry) => entry.phase), [detail]);
  const runNodes = useMemo(
    () => runStepNodes(unifiedRunNodes(detail.runs, activeRun), phaseOrder),
    [detail.runs, activeRun, phaseOrder],
  );
  const runEdges = useMemo(() => unifiedRunEdges(detail.runs, activeRun), [detail.runs, activeRun]);
  const graph = useMemo<FlowGraph>(
    () => buildFlowGraph(detail, runNodes, runEdges),
    [detail, runNodes, runEdges],
  );
  const materials = useMemo<FlowMaterials>(
    () => mountMaterials(detail, artifacts),
    [detail, artifacts],
  );
  const lastRun = detail.runs.length > 0 ? detail.runs[detail.runs.length - 1] : null;
  return { graph, materials, lastRun };
}

/** 详情已载入形态（图数据源经 [`useFlowGraphData`] 单点派生）。 */
function DetailLoaded({
  detail,
  activeRun,
  root,
  id,
  artifacts,
  run,
  header,
  transcriptRefreshKey,
}: DetailLoadedProps): React.JSX.Element {
  const { graph, materials, lastRun } = useFlowGraphData(detail, activeRun, artifacts);
  return (
    <div className="flex flex-col h-full">
      {header}
      <DetailActionPanels
        detail={detail}
        id={id}
        activeRun={activeRun}
        lastRun={lastRun}
        run={run}
      />
      <div className="flex-1 min-h-0">
        <DetailContent
          root={root}
          id={id ?? detail.id}
          name={detail.name}
          graph={graph}
          materials={materials}
          transcriptRefreshKey={transcriptRefreshKey}
        />
      </div>
    </div>
  );
}

/**
 * change 详情视图
 */
/** 显式返回清单（不用历史回退——既有语义）。 */
function useBackToList(): () => void {
  const navigate = useNavigate();
  return useCallback(() => navigate('/changes'), [navigate]);
}

/** 详情页数据面（取数 + 通知分流 + run 控制；参数 id 化）——视图组件薄装配
 *（max-lines 纪律；通知分流单点：D8 kind 位）。归档面已并入 run 流程
 *（archive 相位 + stop-before-archive 停等面），无独立归档链状态源）。 */
function useDetailViewData(root: string | null, id: string | null) {
  const {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refresh,
    notifyRefresh,
    notifyTranscript,
    transcriptTick,
  } = useChangeDetail(root, id);
  // 通知分流单点（D8 kind 位）：会话事件 → 转录重查（150ms 去抖），其余 →
  // 统一视图重查（300ms 去抖）；通知仅失效信号、查询结果权威
  const onNotice = useCallback(
    (kind: RunNotice['ipc']) => {
      if (kind === 'sessionEvent') {
        notifyTranscript();
      } else {
        notifyRefresh();
      }
    },
    [notifyTranscript, notifyRefresh],
  );
  const run = useChangeFlowRun({
    root,
    id,
    activeRunPresent: activeRun !== null,
    onNotice,
  });
  return {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refresh,
    run,
    transcriptTick,
  };
}

export function ChangeDetailView({ root }: { root: string | null }) {
  const params = useParams<'id'>();
  const id = useRootSwitchSuppress(root, params.id ?? null);
  const { detail, activeRun, artifacts, loading, error, refresh, run, transcriptTick } =
    useDetailViewData(root, id);
  const backToList = useBackToList();

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
    <DetailLoaded
      detail={detail}
      activeRun={activeRun}
      root={root}
      id={id}
      artifacts={artifacts}
      run={run}
      header={
        <DetailHeader detail={detail} loading={loading} onBack={backToList} refresh={refresh} />
      }
      transcriptRefreshKey={transcriptTick}
    />
  );
}
