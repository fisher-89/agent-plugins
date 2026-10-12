import { AgentMessages } from '@/components/agent';
import { Badge } from '@/components/ui/badge';
import type { SessionSummary } from '@/types/generated/bindings';

import { useSessionTranscript } from '../hooks/use-session-transcript';
import type { FlowRoleLabel, RoleSessionRef } from './types';

interface SessionTranscriptPanelProps {
  root: string | null;
  roleRef: RoleSessionRef | null;
  /** 变更通知触发的转录重查键（会话事件通知 150ms 去抖自增；实时面统一
   * 转录库重查——liveEvents 退役） */
  refreshKey?: number;
}

/** 会话元信息区 */
function SessionMeta({
  summary,
  running,
}: {
  summary: SessionSummary;
  running: boolean;
}): React.JSX.Element {
  return (
    <div
      className="mb-2 flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 text-[12px] text-muted-foreground"
      data-testid="session-meta"
    >
      <span
        className="max-w-[280px] truncate font-mono"
        title={summary.row.id}
        data-testid="session-meta-id"
      >
        {summary.row.id}
      </span>
      <Badge variant={running ? 'default' : 'secondary'} data-testid="session-meta-status">
        {running ? '运行中' : '已收口'}
      </Badge>
      <span data-testid="session-meta-turns">轮数 {summary.stats.turnCount}</span>
      <span data-testid="session-meta-tokens">
        tokens {summary.stats.inputTokens ?? '—'} / {summary.stats.outputTokens ?? '—'}
      </span>
    </div>
  );
}

/** role 分页转录面板 */
export function SessionTranscriptPanel({
  root,
  roleRef,
  refreshKey,
}: SessionTranscriptPanelProps): React.JSX.Element {
  if (!roleRef) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="drawer-session-empty">
        （当前选中无关联会话）
      </div>
    );
  }
  return (
    <section className="flex h-full min-h-0 flex-col" data-testid="session-transcript-panel">
      <RoleTranscript
        key={roleRef.sessionId ?? roleRef.sourceRef ?? roleRef.role}
        root={root}
        sessionId={roleRef.sessionId}
        sourceRef={roleRef.sourceRef}
        role={roleRef.role}
        refreshKey={refreshKey}
      />
    </section>
  );
}

/** 单 role 转录区 */
function RoleTranscript({
  root,
  sessionId,
  sourceRef,
  role,
  refreshKey,
}: {
  root: string | null;
  sessionId: string | null;
  sourceRef: string | null;
  role: FlowRoleLabel;
  refreshKey?: number;
}): React.JSX.Element {
  const { messages, running, error, summary } = useSessionTranscript({
    root,
    sessionId,
    sourceRef,
    refreshKey,
  });
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-transcript-role={role}>
      {summary !== null && <SessionMeta summary={summary} running={running} />}
      {error !== null && (
        <div
          className="mb-2 shrink-0 break-all text-[13px] text-fail"
          data-testid="transcript-error"
        >
          {error}
        </div>
      )}
      {messages.length === 0 && error === null ? (
        <div className="shrink-0 text-[13px] text-muted-foreground" data-testid="transcript-empty">
          （暂无该会话转录）
        </div>
      ) : (
        <AgentMessages messages={messages} running={running} />
      )}
    </div>
  );
}
