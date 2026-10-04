import { useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';

import { AgentTimeline } from '../../../components/agent';
import type { AgentEvent } from '../../../types/dto';
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

/** role 分页转录面板：多 ref tab 切换、单 ref 直渲染。 */
export function SessionTranscriptPanel({
  root,
  roleRefs,
  liveEvents,
}: SessionTranscriptPanelProps): React.JSX.Element | null {
  const [activeIndex, setActiveIndex] = useState(0);
  // refs 变化（换节点）时回到首页
  useEffect(() => {
    setActiveIndex(0);
  }, [roleRefs]);

  if (roleRefs.length === 0) return null;
  const active = roleRefs[Math.min(activeIndex, roleRefs.length - 1)];
  return (
    <section className="mb-4" data-testid="session-transcript-panel" data-role={active.role}>
      <div className="mb-2 flex flex-wrap items-center gap-1.5">
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

/** 单 role 转录区：寻址 + 重放 + 实时并入（useSessionTranscript）+ 时间线。 */
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
  const { messages, running, error } = useSessionTranscript({
    root,
    sessionId,
    sourceRef,
    liveEvents,
  });
  return (
    <div data-transcript-role={role}>
      {error !== null && (
        <div className="mb-2 break-all text-[13px] text-fail" data-testid="transcript-error">
          {error}
        </div>
      )}
      {messages.length === 0 && error === null ? (
        <div className="text-[13px] text-muted-foreground" data-testid="transcript-empty">
          （暂无该会话转录）
        </div>
      ) : (
        <div className="max-h-[480px] overflow-y-auto rounded-md border border-border bg-background px-3 py-2">
          <AgentTimeline messages={messages} running={running} />
        </div>
      )}
    </div>
  );
}
