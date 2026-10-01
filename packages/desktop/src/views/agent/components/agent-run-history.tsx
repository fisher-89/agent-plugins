import { Button } from '@/components/ui/button';

import { AgentTimeline } from '../../../components/agent';
import { eventsToUIMessages } from '../../../lib/agent-adapter';
import type { AgentRunStatus, SessionSummary } from '../../../types/dto';
import type { AgentRunHistoryState } from '../hooks/use-agent-run-history';

export interface AgentRunHistoryProps {
  state: AgentRunHistoryState;
}

/** 状态文案（受控字符串直出） */
function statusLabel(status: AgentRunStatus): string {
  if (status === 'running') return '运行中';
  if (status === 'completed') return '已完成';
  if (status === 'stopped') return '已停止';
  return '失败';
}

/** 会话终态：最新轮行状态（无轮行的空会话按运行中呈灰） */
function sessionStatus(session: SessionSummary): AgentRunStatus | null {
  const last = session.turns.at(-1);
  return last?.status ?? null;
}

/** 单条会话行：更新时间 / 轮数 / 终态，点开重放全史转录 */
function SessionRow({
  session,
  onOpen,
}: {
  session: SessionSummary;
  onOpen: (sessionId: string) => void;
}) {
  const status = sessionStatus(session);
  return (
    <button
      type="button"
      className="block w-full cursor-pointer border-0 border-b border-b-border bg-transparent px-0 py-2 text-left last:border-b-0 hover:text-primary"
      data-testid="agent-run-row"
      data-session-id={session.row.id}
      data-status={status ?? 'empty'}
      onClick={() => onOpen(session.row.id)}
    >
      <span className="mb-0.5 flex items-center gap-2 text-xs">
        <span
          className={
            status === null
              ? 'text-muted-foreground'
              : status === 'failed'
                ? 'text-fail'
                : status === 'completed'
                  ? 'text-pass'
                  : 'text-muted-foreground'
          }
          data-testid="run-status"
        >
          {status === null ? '空会话' : statusLabel(status)}
        </span>
        <span className="text-muted-foreground">{session.stats.turnCount} 轮</span>
        <span className="text-muted-foreground">
          {new Date(session.row.updatedAt).toLocaleString()}
        </span>
      </span>
      <span className="block truncate text-sm text-muted-foreground">{session.row.id}</span>
      {session.turns.at(-1)?.error != null && (
        <span className="block break-all text-xs text-fail" data-testid="run-row-error">
          {session.turns.at(-1)?.error}
        </span>
      )}
    </button>
  );
}

/**
 * 历史会话区：会话列表（更新时间 / 轮数 / 终态）→ 点开经 invoke 查询重放
 * 全史密封转录（不要求原运行进程存活）+ 显式刷新按钮。会话结束不自动刷新
 * ——列表仅经显式刷新 / 点开取得。重放区限高（max-h-96）内部滚动，不撑高
 * 页面（密封-only `eventsToUIMessages`，delta 仅实时流可见）。
 */
export function AgentRunHistory({ state }: AgentRunHistoryProps): React.JSX.Element {
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-run-history"
    >
      <div className="mb-2 flex items-center justify-between">
        <h2 className="m-0 text-[15px]">
          历史会话 <span className="text-muted-foreground">({state.sessions.length})</span>
        </h2>
        <Button disabled={state.loading} data-testid="history-refresh" onClick={state.refresh}>
          刷新历史
        </Button>
      </div>
      {state.error !== null && (
        <div
          className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="history-error"
        >
          历史加载失败：{state.error}
        </div>
      )}
      {state.loading && (
        <div className="text-muted-foreground" data-testid="history-loading">
          加载中…
        </div>
      )}
      {!state.loading && state.sessions.length === 0 && state.error === null && (
        <div className="text-muted-foreground" data-testid="history-empty">
          暂无历史会话，点击「刷新历史」获取。
        </div>
      )}
      {state.sessions.map((session) => (
        <SessionRow key={session.row.id} session={session} onOpen={state.openSession} />
      ))}
      {state.selectedSessionId !== null && (
        <div
          className="mt-3 max-h-96 overflow-y-auto"
          data-testid="replay-area"
          data-selected-run-id={state.selectedSessionId}
        >
          <div className="mb-1 text-xs text-muted-foreground">重放会话（密封转录）</div>
          <AgentTimeline messages={eventsToUIMessages(state.events)} running={false} />
        </div>
      )}
    </section>
  );
}
