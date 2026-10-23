import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  usePanelRef,
} from '@/components/ui/resizable';
import { Spinner } from '@/components/ui/spinner';

import type {
  ActiveRunView,
  ArtifactEnvelope,
  ChangeDetail,
  ChangeRunEntry,
} from '../../types/dto';
import { ChangeChatPanel } from './agent/change-chat-panel';
import { mountMaterials } from './flow/attachments';
import { ChangeFlowGraph } from './flow/change-flow-graph';
import { DetailDrawer } from './flow/detail-drawer';
import { buildFlowGraph } from './flow/graph';
import { RunControlPanel } from './flow/run-control-panel';
import { runStepNodes, unifiedRunEdges, unifiedRunNodes } from './flow/run-state';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './flow/types';
import { useChangeDetail } from './hooks/use-change-detail';
import { useChangeFlowRun } from './hooks/use-change-flow-run';

/** 左栏（通用 Agent 面板）分栏尺寸（百分比 / rem 字面量，与探索详情页同构） */
const CHAT_PANEL_DEFAULT_SIZE = '30';
const CHAT_PANEL_MIN_SIZE = '22';
/** 收起后留窄条（`collapsed` 时为面板宽度，窄条内呈展开入口） */
const CHAT_PANEL_COLLAPSED_SIZE = '3rem';
/** 折叠判据阈值：`onResize` 回传像素宽 <= 本值即视为收起（3rem = 48px） */
const CHAT_PANEL_RAIL_PX = 48;

function formatTime(value: string | null): string {
  if (value === null) return '—';
  return value;
}

function DetailHeader({
  detail,
  loading,
  chatCollapsed,
  onBack,
  refresh,
  onToggleChat,
}: {
  detail: ChangeDetail;
  loading: boolean;
  chatCollapsed: boolean;
  onBack: () => void;
  refresh: () => void;
  onToggleChat: () => void;
}) {
  return (
    <div className="mb-3" data-testid="detail-header">
      <div className="flex flex-wrap items-center gap-2.5">
        <Button onClick={onBack}>← 返回列表</Button>
        <Button onClick={refresh} disabled={loading} data-testid="refresh-change-detail-button">
          刷新详情
          {loading && <Spinner />}
        </Button>
        <Button data-testid="detail-chat-toggle" onClick={onToggleChat}>
          {chatCollapsed ? '显示通用 Agent' : '隐藏通用 Agent'}
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

/** 内联「刷新失败」横幅（保 last-good 陈旧窗口标示，D1：页首整宽、失败面色
 * 同 run-error / error-note 先例）：失败轮随 `refreshFailed` 呈现，成功轮 /
 * 真 None 轮清 null 即消失；文案含后端错误串原样（层 2 观测面）与「刷新详情」
 * 立即重试引导。 */
function RefreshStaleNote({ message }: { message: string }): React.JSX.Element {
  return (
    <div
      className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-[13px] text-fail"
      data-testid="refresh-stale-note"
    >
      {`刷新失败：${message}——页面显示的是最近一次成功结果，将自动重试；也可点击「刷新详情」立即重试。`}
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
  sessionRefreshKey,
}: {
  root: string | null;
  id: string;
  /** change name（stdout 面板磁盘面寻址键——与 record 同口径，非 id） */
  name: string;
  graph: FlowGraph;
  materials: FlowMaterials;
  sessionRefreshKey: number;
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
        sessionRefreshKey={sessionRefreshKey}
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
  /** 内联「刷新失败」错误串（null = 无陈旧窗口；非 null = 保 last-good 中） */
  staleError: string | null;
  sessionRefreshKey: number;
  /** 左栏分栏命令面（折叠 / 展开经 `panelRef`） */
  chatPanelRef: ReturnType<typeof usePanelRef>;
  /** 左栏折叠态（真相取分栏 `onResize` 回传） */
  chatCollapsed: boolean;
  /** 折叠态回传（`onResize` 按像素宽判定） */
  onChatCollapseChange: (collapsed: boolean) => void;
  /** 折叠 / 展开切换（header 开关与窄条按钮共用同一入口） */
  onToggleChat: () => void;
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

/** 详情已载入形态（图数据源经 [`useFlowGraphData`] 单点派生；header 整宽留在
 * 分栏之上，分栏以下左栏通用 Agent 面板 / 右栏「刷新失败提示 → 运行控制面板 →
 * 流程图区」——D16 右栏次序沿用「陈旧横幅在动作面板之上」）。 */
function DetailLoaded({
  detail,
  activeRun,
  root,
  id,
  artifacts,
  run,
  header,
  staleError,
  sessionRefreshKey,
  chatPanelRef,
  chatCollapsed,
  onChatCollapseChange,
  onToggleChat,
}: DetailLoadedProps): React.JSX.Element {
  const { graph, materials, lastRun } = useFlowGraphData(detail, activeRun, artifacts);
  return (
    <div className="flex flex-col h-full">
      {header}
      <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
        <ResizablePanel
          defaultSize={CHAT_PANEL_DEFAULT_SIZE}
          minSize={CHAT_PANEL_MIN_SIZE}
          collapsible
          collapsedSize={CHAT_PANEL_COLLAPSED_SIZE}
          panelRef={chatPanelRef}
          onResize={(size) => onChatCollapseChange(size.inPixels <= CHAT_PANEL_RAIL_PX)}
        >
          <ChangeChatPanel
            root={root}
            changeId={id ?? detail.id}
            cwd={detail.worktree}
            readOnly={detail.source === 'archive'}
            runActive={activeRun !== null}
            collapsed={chatCollapsed}
            onToggleCollapsed={onToggleChat}
          />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel minSize="20">
          <div className="flex h-full min-h-0 flex-col pl-1.5">
            {staleError !== null && <RefreshStaleNote message={staleError} />}
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
                sessionRefreshKey={sessionRefreshKey}
              />
            </div>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
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

/** 详情页数据面（取数 + 通知归一 + run 控制；参数 id 化）——视图组件薄装配
 *（max-lines 纪律；通知不分流：会话事件与其余同走统一重查 tick）。归档面已
 * 并入 run 流程（archive 相位 + stop-before-archive 停等面），无独立归档链
 * 状态源）。 */
function useDetailViewData(root: string | null, id: string | null) {
  const {
    detail,
    activeRun,
    artifacts,
    loading,
    error,
    refreshFailed,
    refresh,
    notifyRefresh,
    refreshTick,
  } = useChangeDetail(root, id);
  // 通知不分流（D8）：会话事件（SessionEvent）与其余 kind 同走统一视图重查
  // （300ms 去抖）——通知仅失效 / 状态重评估信号、查询结果权威；实时面走会话
  // 订阅（delta 不落库，重查只能见密封粒度）
  const onNotice = useCallback(() => {
    notifyRefresh();
  }, [notifyRefresh]);
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
    refreshFailed,
    refresh,
    run,
    refreshTick,
  };
}

/**
 * 左栏 chat 分栏的命令面与折叠态：折叠态**真相取 Panel 的 `onResize` 回传**
 *（用户拖拽收起同样被捕捉，header 文案与窄条内容不与实际宽度脱节）；`toggle`
 * 依 `panelRef.current.isCollapsed()` 分派 `collapse()` / `expand()`，header 开关
 * 与窄条展开按钮共用同一入口。
 */
function useChatPanelState(): {
  panelRef: ReturnType<typeof usePanelRef>;
  collapsed: boolean;
  toggle: () => void;
  setCollapsed: (collapsed: boolean) => void;
} {
  const panelRef = usePanelRef();
  const [collapsed, setCollapsed] = useState(true);
  const toggle = useCallback(() => {
    const panel = panelRef.current;
    if (panel === null) return;
    if (panel.isCollapsed()) {
      panel.expand();
    } else {
      panel.collapse();
    }
  }, [panelRef]);
  return { panelRef, collapsed, toggle, setCollapsed };
}

export function ChangeDetailView({ root }: { root: string | null }) {
  const params = useParams<'id'>();
  const id = useRootSwitchSuppress(root, params.id ?? null);
  const { detail, activeRun, artifacts, loading, error, refreshFailed, refresh, run, refreshTick } =
    useDetailViewData(root, id);
  const backToList = useBackToList();
  const chatPanel = useChatPanelState();
  const { panelRef: chatPanelRef, toggle: toggleChat } = chatPanel;

  // change id 变化：折叠复位（新 change 的面板以展开态呈现；`onResize` 自动
  // 回传 false，折叠态真相仍单源）
  useEffect(() => {
    const panel = chatPanelRef.current;
    if (panel !== null && !panel.isCollapsed()) panel.collapse();
  }, [id, chatPanelRef]);

  // error 已收敛为首载语义（detail 在手的刷新失败落 refreshFailed 走内联横
  // 幅，不再触整页 DetailFallback）——本分支只承接首载失败（无可保）
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
      staleError={refreshFailed}
      header={
        <DetailHeader
          detail={detail}
          loading={loading}
          chatCollapsed={chatPanel.collapsed}
          onBack={backToList}
          refresh={refresh}
          onToggleChat={toggleChat}
        />
      }
      sessionRefreshKey={refreshTick}
      chatPanelRef={chatPanelRef}
      chatCollapsed={chatPanel.collapsed}
      onChatCollapseChange={chatPanel.setCollapsed}
      onToggleChat={toggleChat}
    />
  );
}
