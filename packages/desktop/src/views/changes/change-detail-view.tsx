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
import { ArchivePanel } from './flow/archive-panel';
import { mountMaterials } from './flow/attachments';
import { ChangeFlowGraph } from './flow/change-flow-graph';
import { DetailDrawer } from './flow/detail-drawer';
import { buildFlowGraph } from './flow/graph';
import { RunControlPanel } from './flow/run-control-panel';
import { runStepNodes, unifiedRunSteps } from './flow/run-state';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './flow/types';
import { useArchiveFlow, type UseArchiveFlowResult } from './hooks/use-archive-flow';
import { useChangeDetail } from './hooks/use-change-detail';
import { useChangeFlowRun } from './hooks/use-change-flow-run';
import { ArtifactTabs } from './renderers/artifact-tabs';

function formatTime(value: string | null): string {
  if (value === null) return '—';
  return value;
}

function DetailHeader({
  detail,
  loading,
  onBack,
  refresh,
  onArchive,
}: {
  detail: ChangeDetail;
  loading: boolean;
  onBack: () => void;
  refresh: () => void;
  onArchive: () => void;
}) {
  return (
    <div className="mb-3" data-testid="detail-header">
      <div className="flex flex-wrap items-center gap-2.5">
        <Button onClick={onBack}>← 返回列表</Button>
        <Button onClick={refresh} disabled={loading}>
          刷新详情
        </Button>
        {/* 归档入口（db status=active 才呈现；已归档不渲染） */}
        {detail.status === 'active' && (
          <Button onClick={onArchive} data-testid="archive-trigger">
            归档…
          </Button>
        )}
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
  graph,
  materials,
  artifacts,
  transcriptRefreshKey,
}: {
  root: string | null;
  id: string;
  graph: FlowGraph;
  materials: FlowMaterials;
  artifacts: ArtifactEnvelope[];
  transcriptRefreshKey: number;
}): React.JSX.Element {
  const [selection, setSelection] = useState<DrawerSelection | null>(null);
  return (
    <>
      <FlowSection graph={graph} materials={materials} onSelect={setSelection} />
      <DetailSectionArtifacts artifacts={artifacts} />
      <DetailDrawer
        selection={selection}
        graph={graph}
        materials={materials}
        root={root}
        changeId={id}
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
  archive: UseArchiveFlowResult;
  header: React.JSX.Element;
  transcriptRefreshKey: number;
  archiveOpen: boolean;
  onArchiveClose: () => void;
}

/** 页内动作面板组（运行控制 + 归档确认；显名收敛：命令面取 URL id、展示面取
 * `detail.name`）。 */
function DetailActionPanels({
  detail,
  root,
  id,
  activeRun,
  lastRun,
  run,
  archive,
  archiveOpen,
  onArchiveClose,
}: {
  detail: ChangeDetail;
  root: string | null;
  id: string | null;
  activeRun: ActiveRunView | null;
  lastRun: ChangeRunEntry | null;
  run: ReturnType<typeof useChangeFlowRun>;
  archive: UseArchiveFlowResult;
  archiveOpen: boolean;
  onArchiveClose: () => void;
}): React.JSX.Element {
  return (
    <>
      {id !== null && detail.status === 'active' && (
        <ArchivePanel
          root={root}
          changeId={id}
          name={detail.name}
          archive={archive}
          open={archiveOpen}
          onClose={onArchiveClose}
        />
      )}
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

/** 详情已载入形态（拼缝单源化：步节点 = runs[].steps ∪ activeRun.steps 同一
 * 转换函数——图常驻渲染，回落分支退场）。 */
function DetailLoaded({
  detail,
  activeRun,
  root,
  id,
  artifacts,
  run,
  archive,
  header,
  transcriptRefreshKey,
  archiveOpen,
  onArchiveClose,
}: DetailLoadedProps): React.JSX.Element {
  const runNodes = useMemo(
    () => runStepNodes(unifiedRunSteps(detail.runs, activeRun?.steps ?? [])),
    [detail.runs, activeRun],
  );
  const graph = useMemo<FlowGraph>(() => buildFlowGraph(detail, runNodes), [detail, runNodes]);
  const materials = useMemo<FlowMaterials>(
    () => mountMaterials(graph, detail, artifacts),
    [detail, graph, artifacts],
  );
  const lastRun = detail.runs.length > 0 ? detail.runs[detail.runs.length - 1] : null;
  return (
    <div>
      {header}
      <DetailActionPanels
        detail={detail}
        root={root}
        id={id}
        activeRun={activeRun}
        lastRun={lastRun}
        run={run}
        archive={archive}
        archiveOpen={archiveOpen}
        onArchiveClose={onArchiveClose}
      />
      <DetailContent
        root={root}
        // 单一身份源 = URL id（detail 恒经该 id 取数、两值同源；不可达形态
        // 兜底记录 id 保取数链断裂时不静默丢身份）
        id={id ?? detail.id}
        graph={graph}
        materials={materials}
        artifacts={artifacts}
        transcriptRefreshKey={transcriptRefreshKey}
      />
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

/** 详情页数据面（取数 + 通知分流 + run 控制 + 归档面；参数 id 化）——视图
 * 组件薄装配（max-lines 纪律；通知分流单点：D8 kind 位）。 */
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
  // 归档面：链终态 onFinish 一次显式 refresh（详情页回落已归档形态、按钮消失）
  const archive = useArchiveFlow({ root, id, onFinish: refresh });
  const [archiveOpen, setArchiveOpen] = useState(false);
  return {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refresh,
    run,
    archive,
    transcriptTick,
    archiveOpen,
    setArchiveOpen,
  };
}

export function ChangeDetailView({ root }: { root: string | null }) {
  const params = useParams<'id'>();
  const id = useRootSwitchSuppress(root, params.id ?? null);
  const {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refresh,
    run,
    archive,
    transcriptTick,
    archiveOpen,
    setArchiveOpen,
  } = useDetailViewData(root, id);
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
      archive={archive}
      header={
        <DetailHeader
          detail={detail}
          loading={loading}
          onBack={backToList}
          refresh={refresh}
          onArchive={() => setArchiveOpen(true)}
        />
      }
      transcriptRefreshKey={transcriptTick}
      archiveOpen={archiveOpen}
      onArchiveClose={() => setArchiveOpen(false)}
    />
  );
}
