/**
 * change 详情左栏通用 Agent 面板（desktop-change-chat）：头区动作面（「新会话」/
 * 「历史 ▾」/ 状态提示行 / 执行根文案）+ `AgentSessionView` 注入形态对话体 +
 * composer 插槽 + 折叠窄条。
 *
 * 纪律：对话与轮行一律经会话级统一组件渲染（面板内零 `eventsToUIMessages` 直
 * 调、零自有转录查询、零消息拼装）；禁发判据只从 `useChangeChat` 取一次并向
 * 下传（`canSend` / `canStart` / `disabledReason`），本文件不做二次判定。
 */
import { AgentSessionView, type AgentSessionActions } from '@/components/agent';
import type { SessionSummary } from '@/types/dto';

import { ChangeChatComposer } from './change-chat-composer';
import { useChangeChat, type ChangeChatState } from './hooks/use-change-chat';

export interface ChangeChatPanelProps {
  /** workspace root（清单与发起的 store 寻址半边） */
  root: string | null;
  /** change id（会话归属 `source_ref`；面板态身份源） */
  changeId: string | null;
  /** 执行根来源（`detail.worktree`；null = legacy 记录回落 root） */
  cwd: string | null;
  /** 归档 change：只读（全禁发、历史仍可浏览） */
  readOnly: boolean;
  /** 在飞 change flow run：禁发判据 */
  runActive: boolean;
  /** 左栏是否处于折叠窄条形态（真相取分栏 `onResize`，视图持有） */
  collapsed: boolean;
  /** 折叠 / 展开切换（沿分栏 `panelRef` 命令面） */
  onToggleCollapsed: () => void;
}

/** 空态（草案态）文案：发送首条消息即开启新会话（不是禁用理由） */
const DRAFT_NOTE = '发送首条消息即开启新会话';

/** 历史清单项 id 前缀长度（完整 id 落 title，行内只呈辨识前缀） */
const HISTORY_ID_PREFIX = 8;

/** 头区动作面：「新会话」（禁发时一并禁）+「历史 ▾」开关（恒可用） */
function ChatHeaderActions({
  canStart,
  listOpen,
  onNewSession,
  onToggleList,
}: {
  canStart: boolean;
  listOpen: boolean;
  onNewSession: () => void;
  onToggleList: () => void;
}): React.JSX.Element {
  return (
    <div className="flex shrink-0 items-center gap-2" data-testid="change-chat-actions">
      <span className="text-[13px] font-medium">通用 Agent</span>
      <span className="flex-1" />
      <button
        type="button"
        className="cursor-pointer rounded-md border border-border bg-transparent px-2.5 py-1 text-xs hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
        data-testid="change-chat-new"
        disabled={!canStart}
        onClick={onNewSession}
      >
        新会话
      </button>
      <button
        type="button"
        className="cursor-pointer rounded-md border border-border bg-transparent px-2.5 py-1 text-xs hover:bg-muted"
        data-testid="change-chat-history"
        aria-expanded={listOpen}
        onClick={onToggleList}
      >
        历史 {listOpen ? '▴' : '▾'}
      </button>
    </div>
  );
}

/** 状态提示行：禁用 / 只读理由优先，否则草案态文案 */
function StatusNote({ text }: { text: string }): React.JSX.Element {
  return (
    <div className="shrink-0 text-[12px] text-muted-foreground" data-testid="change-chat-note">
      {text}
    </div>
  );
}

/** 单条历史会话行：id 前缀 + 更新时间 + 轮数 */
function HistoryItem({
  session,
  onSelect,
}: {
  session: SessionSummary;
  onSelect: (sessionId: string) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className="block w-full cursor-pointer rounded-md border border-border bg-transparent px-2 py-1 text-left text-[12px] hover:bg-muted"
      data-testid="change-chat-history-item"
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
function HistoryList({ state }: { state: ChangeChatState }): React.JSX.Element {
  return (
    <div
      className="max-h-56 shrink-0 overflow-y-auto rounded-md border border-border px-2 py-1.5"
      data-testid="change-chat-history-list"
    >
      {state.listLoading && state.sessions.length === 0 && (
        <div className="text-[12px] text-muted-foreground">加载中…</div>
      )}
      {state.listError !== null && (
        <div className="break-all text-[12px] text-fail" data-testid="change-chat-history-error">
          {`加载历史会话失败：${state.listError}`}
        </div>
      )}
      {!state.listLoading && state.listError === null && state.sessions.length === 0 && (
        <div className="text-[12px] text-muted-foreground" data-testid="change-chat-history-empty">
          （暂无历史会话）
        </div>
      )}
      <div className="flex flex-col gap-1">
        {state.sessions.map((session) => (
          <HistoryItem key={session.row.id} session={session} onSelect={state.selectSession} />
        ))}
      </div>
      {state.sessions.length < state.total && (
        <button
          type="button"
          className="mt-1 w-full cursor-pointer rounded-md border border-border bg-transparent px-2 py-1 text-[12px] hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
          data-testid="change-chat-history-more"
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
function CollapsedRail({ onExpand }: { onExpand: () => void }): React.JSX.Element {
  return (
    <div className="flex h-full flex-col items-center gap-2 py-2" data-testid="change-chat-rail">
      <button
        type="button"
        className="cursor-pointer rounded-md border border-border bg-transparent px-1 py-1 text-xs hover:bg-muted"
        data-testid="change-chat-expand"
        title="展开通用 Agent"
        onClick={onExpand}
      >
        ›
      </button>
      <span className="text-[12px] text-muted-foreground [writing-mode:vertical-rl]">
        通用 Agent
      </span>
    </div>
  );
}

/** 展开态面板体：状态提示 / 执行根文案 / 历史清单 / 对话体 + composer 插槽 */
function ExpandedBody({
  state,
  root,
}: {
  state: ChangeChatState;
  root: string | null;
}): React.JSX.Element {
  // 提示行：禁用理由优先；否则空态（草案态）文案；有会话且无理由时不占位
  const note = state.disabledReason ?? (state.currentSessionId === null ? DRAFT_NOTE : null);
  return (
    <div className="flex h-full min-h-0 flex-col gap-2 pr-1.5">
      <ChatHeaderActions
        canStart={state.canStart}
        listOpen={state.listOpen}
        onNewSession={state.newSession}
        onToggleList={state.toggleList}
      />
      {note !== null && <StatusNote text={note} />}
      <div
        className="shrink-0 break-all text-[11px] text-muted-foreground"
        data-testid="change-chat-cwd"
      >
        {`执行根：${state.cwd ?? root ?? '—'}`}
      </div>
      {state.listOpen && <HistoryList state={state} />}
      <AgentSessionView
        session={state.session}
        composer={({ running }: AgentSessionActions) => (
          <ChangeChatComposer
            running={running}
            disabled={!state.canSend}
            disabledReason={state.disabledReason}
            onSend={state.send}
            onStop={state.stop}
          />
        )}
        data-testid="change-chat-session"
      />
    </div>
  );
}

/**
 * change 详情左栏通用 Agent 面板。会话状态 / 清单 / 判据全在 `useChangeChat`
 *（面板态挂 change id，切换即复位）；折叠态只渲染窄条（真相取分栏 onResize）。
 */
export function ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element {
  const { root, changeId, cwd, readOnly, runActive, collapsed, onToggleCollapsed } = props;
  const state = useChangeChat({
    root,
    changeId,
    worktree: cwd,
    runActive,
    archived: readOnly,
  });
  return (
    <div className="h-full min-h-0" data-testid="change-chat-panel">
      {collapsed ? (
        <CollapsedRail onExpand={onToggleCollapsed} />
      ) : (
        <ExpandedBody state={state} root={root} />
      )}
    </div>
  );
}
