import { Button } from '@/components/ui/button';

import type { AgentRunRecord, AgentRunStatus } from '../../../types/dto';
import type { AgentRunHistoryState } from '../hooks/useAgentRunHistory';
import { AgentEventTimeline } from './AgentEventTimeline';

export interface AgentRunHistoryProps {
  state: AgentRunHistoryState;
}

/** 状态文案（受控字符串直出） */
function statusLabel(status: AgentRunStatus): string {
  return status === 'running' ? '运行中' : status === 'completed' ? '已完成' : '失败';
}

/** 单条 run 行：状态 / 时间 / 提示词摘要，点开重放 */
function RunRow({ run, onOpen }: { run: AgentRunRecord; onOpen: (runId: number) => void }) {
  return (
    <button
      type="button"
      className="block w-full cursor-pointer border-0 border-b border-b-border bg-transparent px-0 py-2 text-left last:border-b-0 hover:text-primary"
      data-testid="agent-run-row"
      data-run-id={run.id}
      data-status={run.status}
      onClick={() => onOpen(run.id)}
    >
      <span className="mb-0.5 flex items-center gap-2 text-xs">
        <span
          className={
            run.status === 'failed'
              ? 'text-fail'
              : run.status === 'completed'
                ? 'text-pass'
                : 'text-muted-foreground'
          }
          data-testid="run-status"
        >
          {statusLabel(run.status)}
        </span>
        <span className="text-muted-foreground">#{run.id}</span>
        <span className="text-muted-foreground">{new Date(run.startedAt).toLocaleString()}</span>
        {run.numTurns !== null && <span className="text-muted-foreground">{run.numTurns} 轮</span>}
      </span>
      <span className="block truncate text-sm">{run.prompt}</span>
      {run.error !== null && (
        <span className="block break-all text-xs text-fail" data-testid="run-row-error">
          {run.error}
        </span>
      )}
    </button>
  );
}

/**
 * 历史运行区：run 列表（状态 / 时间 / 提示词摘要）→ 点开经 invoke 查询重放
 * 落库事件（不要求原运行进程存活）+ 显式刷新按钮。run 结束不自动刷新——
 * 列表仅经显式刷新 / 点开取得。
 */
export function AgentRunHistory({ state }: AgentRunHistoryProps): React.JSX.Element {
  return (
    <section
      className="rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="agent-run-history"
    >
      <div className="mb-2 flex items-center justify-between">
        <h2 className="m-0 text-[15px]">
          历史运行 <span className="text-muted-foreground">({state.runs.length})</span>
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
      {!state.loading && state.runs.length === 0 && state.error === null && (
        <div className="text-muted-foreground" data-testid="history-empty">
          暂无历史运行，点击「刷新历史」获取。
        </div>
      )}
      {state.runs.map((run) => (
        <RunRow key={run.id} run={run} onOpen={state.openRun} />
      ))}
      {state.selectedRunId !== null && (
        <div className="mt-3" data-testid="replay-area" data-selected-run-id={state.selectedRunId}>
          <div className="mb-1 text-xs text-muted-foreground">
            重放 run #{state.selectedRunId}（落库事件）
          </div>
          <AgentEventTimeline events={state.events} running={false} />
        </div>
      )}
    </section>
  );
}
