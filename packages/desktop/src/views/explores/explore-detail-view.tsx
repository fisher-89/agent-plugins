import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router';

import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

import { AgentSessionView, type AgentSessionActions } from '../../components/agent';
import { extractMarkdownTitle } from '../../lib/explore-title';
import { extractWorkflowType } from '../../lib/explore-workflow-type';
import type { ExploreRecord } from '../../types/dto';
import { ExploreComposer } from './components/explore-composer';
import { ExplorePreview } from './components/explore-preview';
import { useExploreDoc } from './hooks/use-explore-doc';
import type { ExploreListState } from './hooks/use-explore-list';
import { useExploreSession } from './hooks/use-explore-session';

export interface ExploreDetailViewProps {
  /** 当前 workspace root（cwd 与读面入参） */
  root: string;
  /** 当前记录（父层按 URL name 从清单派生——id 桥三级链，旧名 miss 以快照
   * 兜底；清单未就绪且无快照时为 null） */
  record: ExploreRecord | null;
  /** 清单状态（promote / updateTitle / refresh 动作来源；与清单同源） */
  list: ExploreListState;
}

/**
 * location.state 宽容解析 autoPrompt（非 string 一律 null——路由 state 无
 * schema 保证，back/forward 复现形态不可信）。
 */
function readAutoPrompt(state: unknown): string | null {
  if (state !== null && typeof state === 'object' && 'autoPrompt' in state) {
    const value = state.autoPrompt;
    if (typeof value === 'string') return value;
  }
  return null;
}

/**
 * 标题回填：消费 watch 防抖 / 终态定点重读后的**稳定内容**，解析首行
 * `# <标题>`，与 `record.title` 不同则经显式写命令 `update_explore_title`
 * 回填（读路径 `read_explore` 零写入）。解析失败（无标题行）或无差异 → 零
 * invoke；回填失败静默（record.title 保持原值，下次内容变化再试，不打扰会话）。
 */
function useTitleBackfill(
  content: string | null,
  recordName: string | null,
  currentTitle: string | null,
  promotedTo: string | null,
  updateTitle: (name: string, title: string) => Promise<void>,
): void {
  useEffect(() => {
    if (recordName === null || currentTitle === null || promotedTo !== null || content === null) {
      return;
    }
    const parsed = extractMarkdownTitle(content);
    if (parsed === null || parsed === currentTitle) return;
    updateTitle(recordName, parsed).catch(() => {
      // 回填失败静默：record.title 保持原值，下次内容变化再试
    });
  }, [content, recordName, currentTitle, promotedTo, updateTitle]);
}

/** promote 动作：在飞标记（防重复提交）、类型选择器本地态与行内错误（后端 Err 原文） */
function usePromoteAction(
  record: ExploreRecord | null,
  promote: (name: string, workflowType: string) => Promise<void>,
  updateWorkflowType: (name: string, workflowType: string) => Promise<void>,
): {
  promoting: boolean;
  promoteError: string | null;
  workflowType: string;
  onWorkflowTypeChange: (next: string) => void;
  onPromote: () => void;
} {
  const [promoting, setPromoting] = useState(false);
  const [promoteError, setPromoteError] = useState<string | null>(null);
  // 选择器默认值 = `record.workflowType`（agent 推算值经 `extractWorkflowType`
  // 回填落库）；推算 / 人工改选后随记录值跟随（本地态即选择器现值）
  const stored = record?.workflowType ?? 'requirement';
  const [workflowType, setWorkflowType] = useState(stored);
  useEffect(() => {
    setWorkflowType(stored);
  }, [stored]);
  // 改即写：人工改选经显式写命令落库（沿 title 改即写模式；失败静默，选择器
  // 保持用户当前选择，下次进详情回落到库内值）
  const onWorkflowTypeChange = useCallback(
    (next: string) => {
      setWorkflowType(next);
      if (record === null) return;
      updateWorkflowType(record.name, next).catch(() => {
        // 回填失败静默：库内值保持原值，下次进详情回落到库内值
      });
    },
    [record, updateWorkflowType],
  );
  const onPromote = useCallback(() => {
    if (record === null || promoting) return;
    setPromoting(true);
    setPromoteError(null);
    promote(record.name, workflowType)
      .catch((err: unknown) => setPromoteError(String(err)))
      .finally(() => setPromoting(false));
  }, [record, promoting, workflowType, promote]);
  return { promoting, promoteError, workflowType, onWorkflowTypeChange, onPromote };
}

/**
 * 类型回填：消费 watch 防抖 / 终态定点重读后的**稳定内容**，解析约定标记行
 * `workflow_type: <requirement|test-only>`，与 `record.workflowType` 不同则经
 * 显式写命令 `update_explore_workflow_type` 回填（读路径 `read_explore` 零写入）。
 * 解析失败（无标记行 / 非法值）或无差异 → 零 invoke；回填失败静默
 * （`record.workflowType` 保持原值，用户可在 promote 入口改选）。
 */
function useWorkflowTypeBackfill(
  content: string | null,
  recordName: string | null,
  currentWorkflowType: string | null,
  promotedTo: string | null,
  updateWorkflowType: (name: string, workflowType: string) => Promise<void>,
): void {
  useEffect(() => {
    if (
      recordName === null ||
      currentWorkflowType === null ||
      promotedTo !== null ||
      content === null
    ) {
      return;
    }
    const parsed = extractWorkflowType(content);
    if (parsed === null || parsed === currentWorkflowType) return;
    updateWorkflowType(recordName, parsed).catch(() => {
      // 回填失败静默：record.workflowType 保持原值，下次内容变化再试
    });
  }, [content, recordName, currentWorkflowType, promotedTo, updateWorkflowType]);
}

/** 记录面回填单点：title 与 workflow_type 两路解析回填（均消费 watch 稳定内容、
 * 均静默失败——解析失败 / 无差异零 invoke）。 */
function useRecordBackfill(
  record: ExploreRecord | null,
  promotedTo: string | null,
  content: string | null,
  list: ExploreListState,
): void {
  useTitleBackfill(
    content,
    record?.name ?? null,
    record?.title ?? null,
    promotedTo,
    list.updateTitle,
  );
  useWorkflowTypeBackfill(
    content,
    record?.name ?? null,
    record?.workflowType ?? null,
    promotedTo,
    list.updateWorkflowType,
  );
}

/** promoted 态动作面：「已转变更」徽标 + 「查看变更」跳转（恒 id 寻址） */
function PromotedActions({ onJump }: { onJump: () => void }): React.JSX.Element {
  return (
    <>
      <span
        className="rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground"
        data-testid="explore-promoted-badge"
      >
        已转变更
      </span>
      <button
        type="button"
        className="cursor-pointer rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted"
        data-testid="explore-promoted-jump"
        onClick={onJump}
      >
        查看变更
      </button>
    </>
  );
}

/** 草稿态动作面：工作流类型选择器（两字面选项普通 select，默认 = 推算值，改即写
 * 落库）+「启动变更」入口 */
function DraftActions({
  promoting,
  workflowType,
  onWorkflowTypeChange,
  onPromote,
}: {
  promoting: boolean;
  workflowType: string;
  onWorkflowTypeChange: (next: string) => void;
  onPromote: () => void;
}): React.JSX.Element {
  return (
    <>
      <select
        className="cursor-pointer rounded-md border border-border bg-transparent px-1.5 py-0.5 text-sm"
        data-testid="explore-workflow-type-select"
        value={workflowType}
        onChange={(event) => onWorkflowTypeChange(event.target.value)}
      >
        <option value="requirement">requirement</option>
        <option value="test-only">test-only</option>
      </select>
      <button
        type="button"
        className="cursor-pointer rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
        data-testid="explore-promote-trigger"
        disabled={promoting}
        onClick={onPromote}
      >
        启动变更
      </button>
    </>
  );
}

/**
 * 详情头：渲染 `title`（人类可读标题，恒非空——无空态回退分支）+ 变更动作面。
 * `promotedTo` 为空即草稿态：「工作流类型选择器」+「启动变更」入口 + 行内错误块；
 * 非空即 promoted 态：「已转变更」徽标 + 「查看变更」跳转（`/changes/<change_id>`，
 * MUST NOT 自动导航离开）。
 */
function ExploreDetailHeader({
  record,
  onJump,
  promoting,
  promoteError,
  workflowType,
  onWorkflowTypeChange,
  onPromote,
}: {
  record: ExploreRecord;
  onJump: () => void;
  promoting: boolean;
  promoteError: string | null;
  workflowType: string;
  onWorkflowTypeChange: (next: string) => void;
  onPromote: () => void;
}): React.JSX.Element {
  const promoted = record.promotedTo !== null;
  return (
    <div className="mb-3 flex flex-wrap items-center gap-2.5" data-testid="explore-detail-header">
      <h2 className="m-0 break-all text-[17px]" data-testid="explore-detail-title">
        {record.title}
      </h2>
      {promoted ? (
        <PromotedActions onJump={onJump} />
      ) : (
        <DraftActions
          onPromote={onPromote}
          onWorkflowTypeChange={onWorkflowTypeChange}
          promoting={promoting}
          workflowType={workflowType}
        />
      )}
      {promoteError !== null && (
        <div
          className="w-full break-all rounded-md bg-fail-bg px-3 py-2 text-sm text-fail"
          data-testid="explore-promote-error"
        >
          启动变更失败：{promoteError}
        </div>
      )}
    </div>
  );
}

/** promoted 态右栏：笔记已随变更移走，不读 `explores/<name>.md`（停 read / watch） */
function PromotedPreview(): React.JSX.Element {
  return (
    <section
      className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card"
      data-testid="explore-promoted-preview"
    >
      <div className="border-b border-border px-3 py-2">
        <h2 className="m-0 text-[15px]">笔记预览</h2>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3 text-muted-foreground">
        该探索已转变为变更，笔记已随变更移走（可在变更详情查看其 explore.md）。
      </div>
    </section>
  );
}

/** 左栏：对话区（会话级组件注入形态 + composer 插槽） */
function ChatPane({
  session,
}: {
  session: ReturnType<typeof useExploreSession>;
}): React.JSX.Element {
  return (
    <div className="flex h-full min-h-0 flex-col gap-3 pr-1.5">
      <AgentSessionView
        session={session.session}
        composer={({ running }: AgentSessionActions) => (
          <ExploreComposer running={running} onSend={session.send} onStop={session.stop} />
        )}
        data-testid="explore-chat"
      />
    </div>
  );
}

/** 右栏：草稿态笔记预览（随 watch 刷新）/ promoted 态提示面板 */
function PreviewPane({
  doc,
  promoted,
}: {
  doc: ReturnType<typeof useExploreDoc>;
  promoted: boolean;
}): React.JSX.Element {
  return (
    <div className="h-full min-h-0 pl-1.5">
      {promoted ? (
        <PromotedPreview />
      ) : (
        <ExplorePreview doc={doc.doc} loading={doc.loading} onRefresh={doc.refresh} />
      )}
    </div>
  );
}

/** 记录未就绪（清单未加载 / 记录不存在）占位 */
function MissingRecord(): React.JSX.Element {
  return (
    <div className="text-muted-foreground" data-testid="explore-detail-missing">
      记录不存在或清单尚未就绪。
    </div>
  );
}

/**
 * 探索详情页（双栏）：左对话区、右文档预览，resizable 可拖分界。文档取数与
 * watch 订阅收口 useExploreDoc（订阅生命周期即页面生命周期，卸载即退订）；
 * 会话链还原 / 续话 / auto-send 收口 useExploreSession；run 终态（running
 * true→false）定点重读文档并刷新清单（id 桥数据半边——记录可能已 in-place
 * 改名），随后名变收敛 effect 把 URL replace 至新名（id 桥导航半边，
 * `useExploreDoc` 以 name 依赖自动重订阅）；title 与 workflow_type 回填见
 * useRecordBackfill。
 */
export function ExploreDetailView({
  root,
  record,
  list,
}: ExploreDetailViewProps): React.JSX.Element {
  const navigate = useNavigate();
  const location = useLocation();
  const { name: routeName } = useParams<'name'>();
  const promotedTo = record?.promotedTo ?? null;
  // 已 promote：笔记已搬入 change 目录，停读 / 停 watch（name=null 早退）
  const name = record !== null && promotedTo === null ? record.name : null;
  const doc = useExploreDoc(root, name);
  const autoPrompt = readAutoPrompt(location.state);
  const session = useExploreSession(root, record, autoPrompt);
  const wasRunning = useRef(false);
  const { promoting, promoteError, workflowType, onWorkflowTypeChange, onPromote } =
    usePromoteAction(record, list.promote, list.updateWorkflowType);

  useRecordBackfill(record, promotedTo, doc.doc?.content ?? null, list);

  // run 终态定点重读文档（含本轮 agent 落盘 / 更新笔记的情形）+ 刷新清单
  // （id 桥数据半边）；挂载不触发。运行标记取库态（轮行含 running）
  const running = session.session.running;
  useEffect(() => {
    if (wasRunning.current && !running) {
      doc.refresh();
      list.refresh();
    }
    wasRunning.current = running;
  }, [running, doc.refresh, list.refresh]);

  // 名变收敛（id 桥导航半边）：派生记录名与 URL 名分叉（create_explore_note
  // in-place 改名后按 id 重寻得新名）→ replace 至新名路径（不携带 state——
  // autoPrompt 已消费或被 chain 闸门拦截）
  useEffect(() => {
    if (record !== null && routeName !== undefined && record.name !== routeName) {
      void navigate(`/explores/${record.name}`, { replace: true });
    }
  }, [record, routeName, navigate]);

  if (record === null) return <MissingRecord />;

  return (
    <div className="h-full flex flex-col" data-testid="explore-detail">
      <ExploreDetailHeader
        onJump={() => void navigate(`/changes/${record.promotedTo}`)}
        onPromote={onPromote}
        onWorkflowTypeChange={onWorkflowTypeChange}
        promoteError={promoteError}
        promoting={promoting}
        record={record}
        workflowType={workflowType}
      />
      <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
        <ResizablePanel defaultSize="55" minSize="25">
          <ChatPane session={session} />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize="45" minSize="20">
          <PreviewPane doc={doc} promoted={promotedTo !== null} />
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
