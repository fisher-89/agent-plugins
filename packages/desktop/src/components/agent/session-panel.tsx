/**
 * 会话面板骨架（`desktop-change-chat`「面板骨架归一」）：change 详情左栏与架构页
 * 左栏的**唯一**面板实现——头区动作（新会话 / 历史 ▾）/ 状态行 / 历史清单（分页
 * + 加载 / 空 / 错误态）/ 折叠窄条 / `AgentSessionView` 注入形态 + composer 插槽。
 *
 * 三条纪律：
 * 1. **业务判据不外移**：可发送 / 只读 / 归档 / 运行期矩阵与状态行文案由来源侧
 *    hook 算好（`canStart` / `note`），骨架 MUST NOT 自判禁发、MUST NOT 自建会话流
 *    （零 `useAgentSession` 调用）；
 * 2. **testid 前缀参数化**：全部查询挂钩由 `testIdPrefix` 生成，骨架内零写死前缀；
 * 3. **折叠入参可选**：`collapsed` 与 `onToggleCollapsed` 同时在场才可能呈窄条；
 *    不传即恒展开（无会话管理维度的消费面零变化）。
 */
import { HistoryIcon, PlusCircleIcon } from 'lucide-react';

import { Button } from '@/components/ui/button';
import type { UseAgentSessionState } from '@/hooks/use-agent-session';
import type { SessionSummary } from '@/types/generated/bindings';

import { AgentSessionView, type AgentSessionActions } from './agent-session-view';

/** 两处面板的公共状态子集（会话面 + 清单面 + 动作面判据 + 状态行） */
export interface SessionPanelState {
  /** 注入形态会话面（`AgentSessionView` 的 `session` 入参） */
  session: UseAgentSessionState;
  /** 历史清单本页项（`updated_at` 降序由查询面保证，分页累加） */
  sessions: SessionSummary[];
  /** 过滤集全量计数（「加载更多」可见条件 `sessions.length < total`） */
  total: number;
  /** 历史清单展开态（展开即重取首屏，收起不清空） */
  listOpen: boolean;
  listLoading: boolean;
  listError: string | null;
  toggleList: () => void;
  loadMore: () => void;
  newSession: () => void;
  selectSession: (sessionId: string) => void;
  /** 「新会话」可用性单源（骨架唯一的判据消费点） */
  canStart: boolean;
  /** 状态行文案（`null` = 不占位；文案生成规则由来源侧 hook 持有） */
  note: string | null;
}

interface SessionPanelProps {
  state: SessionPanelState;
  /** 面板标题（头区标签 + 窄条竖向标题） */
  title: string;
  /** 全部查询挂钩的前缀（归属标识由消费方给出，骨架内零写死前缀） */
  testIdPrefix: string;
  /** composer 插槽（注入形态：插槽自取运行态与动作面） */
  composer: (actions: AgentSessionActions) => React.ReactNode;
  /** 折叠态（缺省 `undefined` = 恒展开；须与 `onToggleCollapsed` 同时在场） */
  collapsed?: boolean;
  /** 折叠 / 展开切换（缺省 `undefined` = 不呈展开入口） */
  onToggleCollapsed?: () => void;
}

/** 历史清单项 id 前缀长度（完整 id 落 title，行内只呈辨识前缀） */
const HISTORY_ID_PREFIX = 8;

/** 查询挂钩单点：`${前缀}-${名}`（骨架内零写死前缀） */
function testId(prefix: string, name: string): string {
  return `${prefix}-${name}`;
}

/** 头区动作面：「新会话」（禁发时一并禁）+「历史 ▾」开关（恒可用） */
function HeaderActions({
  title,
  testIdPrefix,
  canStart,
  listOpen,
  onNewSession,
  onToggleList,
}: {
  title: string;
  testIdPrefix: string;
  canStart: boolean;
  listOpen: boolean;
  onNewSession: () => void;
  onToggleList: () => void;
}): React.JSX.Element {
  return (
    <div className="flex shrink-0 items-center gap-2" data-testid={testId(testIdPrefix, 'actions')}>
      <span className="text-[13px] font-medium">{title}</span>
      <span className="flex-1" />
      <Button
        variant="link"
        size="icon"
        aria-label="新会话"
        data-testid={testId(testIdPrefix, 'new')}
        disabled={!canStart}
        onClick={onNewSession}
      >
        <PlusCircleIcon />
      </Button>
      <Button
        variant="link"
        size="icon"
        aria-label="历史会话"
        data-testid={testId(testIdPrefix, 'history')}
        aria-expanded={listOpen}
        onClick={onToggleList}
      >
        <HistoryIcon />
      </Button>
    </div>
  );
}

/** 单条历史会话行：id 前缀 + 更新时间 + 轮数 */
function HistoryItem({
  session,
  testIdPrefix,
  onSelect,
}: {
  session: SessionSummary;
  testIdPrefix: string;
  onSelect: (sessionId: string) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className="block w-full cursor-pointer rounded-md border border-border bg-transparent px-2 py-1 text-left text-[12px] hover:bg-muted"
      data-testid={testId(testIdPrefix, 'history-item')}
      data-session-id={session.row.id}
      title={session.row.id}
      onClick={() => onSelect(session.row.id)}
    >
      <span className="block font-mono">{session.row.id.slice(0, HISTORY_ID_PREFIX)}</span>
      <span className="block text-muted-foreground">
        {new Date(session.row.updatedAt).toLocaleString()} · {session.stats.turnCount} 轮
      </span>
    </button>
  );
}

/** 历史清单（展开态）：加载 / 错误 / 空态文案 + 分页累加（`items < total`） */
function HistoryList({
  state,
  testIdPrefix,
}: {
  state: SessionPanelState;
  testIdPrefix: string;
}): React.JSX.Element {
  return (
    <div
      className="max-h-56 shrink-0 overflow-y-auto rounded-md border border-border px-2 py-1.5"
      data-testid={testId(testIdPrefix, 'history-list')}
    >
      {state.listLoading && state.sessions.length === 0 && (
        <div className="text-[12px] text-muted-foreground">加载中…</div>
      )}
      {state.listError !== null && (
        <div
          className="break-all text-[12px] text-fail"
          data-testid={testId(testIdPrefix, 'history-error')}
        >
          {`加载历史会话失败：${state.listError}`}
        </div>
      )}
      {!state.listLoading && state.listError === null && state.sessions.length === 0 && (
        <div
          className="text-[12px] text-muted-foreground"
          data-testid={testId(testIdPrefix, 'history-empty')}
        >
          （暂无历史会话）
        </div>
      )}
      <div className="flex flex-col gap-1">
        {state.sessions.map((session) => (
          <HistoryItem
            key={session.row.id}
            session={session}
            testIdPrefix={testIdPrefix}
            onSelect={state.selectSession}
          />
        ))}
      </div>
      {state.sessions.length < state.total && (
        <button
          type="button"
          className="mt-1 w-full cursor-pointer rounded-md border border-border bg-transparent px-2 py-1 text-[12px] hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
          data-testid={testId(testIdPrefix, 'history-more')}
          disabled={state.listLoading}
          onClick={state.loadMore}
        >
          加载更多
        </button>
      )}
    </div>
  );
}

/** 折叠窄条：竖向标题 + 展开入口（收起态仍留可见入口） */
function CollapsedRail({
  title,
  testIdPrefix,
  onExpand,
}: {
  title: string;
  testIdPrefix: string;
  onExpand: () => void;
}): React.JSX.Element {
  return (
    <div
      className="flex h-full flex-col items-center gap-2 py-2"
      data-testid={testId(testIdPrefix, 'rail')}
    >
      <button
        type="button"
        className="cursor-pointer rounded-md border border-border bg-transparent px-1 py-1 text-xs hover:bg-muted"
        data-testid={testId(testIdPrefix, 'expand')}
        title={`展开${title}`}
        onClick={onExpand}
      >
        ›
      </button>
      <span className="text-[12px] text-muted-foreground [writing-mode:vertical-rl]">{title}</span>
    </div>
  );
}

/**
 * 会话面板骨架：`isCollapsed` 为真只渲窄条（无展开入口的死锁形态被反向条件排
 * 除）；否则渲展开体——头区动作 → 状态行（`state.note`）→ 历史清单 → 会话体。
 * 状态行文案生成规则不在本组件内（来源侧 hook 供 `note`）。
 */
export function SessionPanel(props: SessionPanelProps): React.JSX.Element {
  const { state, title, testIdPrefix, composer, collapsed, onToggleCollapsed } = props;
  const isCollapsed = collapsed === true && onToggleCollapsed !== undefined;
  return (
    <div className="h-full min-h-0" data-testid={testId(testIdPrefix, 'panel')}>
      {isCollapsed ? (
        <CollapsedRail title={title} testIdPrefix={testIdPrefix} onExpand={onToggleCollapsed} />
      ) : (
        <ExpandedBody state={state} title={title} testIdPrefix={testIdPrefix} composer={composer} />
      )}
    </div>
  );
}

/** 展开态面板体：头区动作 → 状态行 → 历史清单 → 会话体 + composer 插槽 */
function ExpandedBody({
  state,
  title,
  testIdPrefix,
  composer,
}: {
  state: SessionPanelState;
  title: string;
  testIdPrefix: string;
  composer: (actions: AgentSessionActions) => React.ReactNode;
}): React.JSX.Element {
  return (
    <div className="flex h-full min-h-0 flex-col gap-2 pr-1.5">
      <HeaderActions
        title={title}
        testIdPrefix={testIdPrefix}
        canStart={state.canStart}
        listOpen={state.listOpen}
        onNewSession={state.newSession}
        onToggleList={state.toggleList}
      />
      {state.note !== null && (
        <div
          className="shrink-0 text-[12px] text-muted-foreground"
          data-testid={testId(testIdPrefix, 'note')}
        >
          {state.note}
        </div>
      )}
      {state.listOpen && <HistoryList state={state} testIdPrefix={testIdPrefix} />}
      <AgentSessionView
        session={state.session}
        composer={composer}
        data-testid={testId(testIdPrefix, 'session')}
      />
    </div>
  );
}
