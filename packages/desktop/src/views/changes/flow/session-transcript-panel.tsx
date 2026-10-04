import { useEffect, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import { AgentTimeline } from '../../../components/agent';
import type { AgentEvent } from '../../../types/dto';
import type { SessionSummary } from '../../../types/generated/bindings';
import { useSessionTranscript } from '../hooks/use-session-transcript';
import type { FlowRoleLabel, RoleSessionRef } from './types';

/** role 分页文案。 */
const ROLE_LABEL: Record<FlowRoleLabel, string> = {
  executor: '执行会话',
  evaluator: '评估会话',
  decision: '决策会话',
};

interface SessionTranscriptPanelProps {
  root: string | null;
  roleRefs: RoleSessionRef[];
  /** 选中会话的实时事件流（运行中并入；收口重放时为空） */
  liveEvents: AgentEvent[];
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
      <Badge variant={running ? 'active' : 'inv2'} data-testid="session-meta-status">
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
  roleRefs,
  liveEvents,
}: SessionTranscriptPanelProps): React.JSX.Element {
  const [activeIndex, setActiveIndex] = useState(0);
  // refs 变化（换节点）时回到首页
  useEffect(() => {
    setActiveIndex(0);
  }, [roleRefs]);

  if (roleRefs.length === 0) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="drawer-session-empty">
        （当前选中无关联会话）
      </div>
    );
  }
  const active = roleRefs[Math.min(activeIndex, roleRefs.length - 1)];
  return (
    <section
      className="flex h-full min-h-0 flex-col"
      data-testid="session-transcript-panel"
      data-role={active.role}
    >
      <div className="mb-2 flex shrink-0 flex-wrap items-center gap-1.5">
        <h3 className="m-0 text-[13px] text-muted-foreground">会话转录</h3>
        {roleRefs.length > 1 &&
          roleRefs.map((ref, index) => (
            <Button
              key={ref.sessionId ?? ref.sourceRef ?? ref.role}
              onClick={() => setActiveIndex(index)}
              data-testid="transcript-role-tab"
            >
              {ROLE_LABEL[ref.role]}
            </Button>
          ))}
      </div>
      <RoleTranscript
        key={active.sessionId ?? active.sourceRef ?? active.role}
        root={root}
        sessionId={active.sessionId}
        sourceRef={active.sourceRef}
        role={active.role}
        liveEvents={liveEvents}
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
  liveEvents,
}: {
  root: string | null;
  sessionId: string | null;
  sourceRef: string | null;
  role: FlowRoleLabel;
  liveEvents: AgentEvent[];
}): React.JSX.Element {
  const { messages, running, error, summary } = useSessionTranscript({
    root,
    sessionId,
    sourceRef,
    liveEvents,
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
        <div className="min-h-0 flex-1 overflow-y-auto rounded-md border border-border bg-background px-3 py-2">
          <AgentTimeline messages={messages} running={running} />
        </div>
      )}
    </div>
  );
}
