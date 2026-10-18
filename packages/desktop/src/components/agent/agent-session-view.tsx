import { Badge } from '@/components/ui/badge';
import {
  useAgentSession,
  type AgentSessionSendInput,
  type UseAgentSessionState,
} from '@/hooks/use-agent-session';
import type { SessionSummary } from '@/types/generated/bindings';

import { AgentMessages } from './agent-messages';

/** composer 插槽入参：运行态 + 动作面（插槽内容据此呈现发送 / 停止） */
export interface AgentSessionActions {
  running: boolean;
  loading: boolean;
  send: (input: AgentSessionSendInput) => void;
  stop: () => void;
}

export interface AgentSessionViewProps {
  /** workspace root（自持形态的取数 / 订阅寻址；注入形态忽略） */
  root?: string | null;
  /** 会话唯一寻址键；`null` 即无会话（渲染空态，零取数零订阅） */
  sessionId?: string | null;
  /** 会话状态重评估信号（变化即重评估是否挂流） */
  refreshKey?: number;
  /** 注入形态：来源侧已建 hook 的会话状态面（在场即注入形态，否则自持形态） */
  session?: UseAgentSessionState;
  /** composer 插槽（render prop：插槽自足取运行态与动作面；只读消费点不传） */
  composer?: (actions: AgentSessionActions) => React.ReactNode;
  'data-testid'?: string;
}

/** 会话元信息区（会话 id / 运行中徽章 / 轮数 / tokens；自组件状态读取） */
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

/** 会话槽位缺席占位（槽位缺席 ≠ 虚构会话：零取数、零订阅） */
function SessionAbsent(): React.JSX.Element {
  return (
    <div className="text-[13px] text-muted-foreground" data-testid="agent-session-empty">
      （暂无会话）
    </div>
  );
}

/**
 * 会话级唯一渲染入口：元信息行 + 错误横幅 + `AgentMessages` 对话透镜 +
 * composer 插槽。双形态共用同一渲染体与同一 `useAgentSession`：
 * - 自持形态（`root` + `sessionId`）：内部建流，供只读消费点（change 抽屉 /
 *   历史重放）使用；
 * - 注入形态（`session`）：来源侧已建 hook 的交互页使用；内部 hook 以
 *   `sessionId = null` 空载（零取数、零订阅、零第二条流）。
 */
export function AgentSessionView(props: AgentSessionViewProps): React.JSX.Element {
  const { root, sessionId, refreshKey, session, composer } = props;
  const injected = session !== undefined;
  const internal = useAgentSession({
    root: injected ? null : (root ?? null),
    sessionId: injected ? null : (sessionId ?? null),
    source: 'debug',
    refreshKey: injected ? undefined : refreshKey,
  });
  const state = session ?? internal;
  const testId = props['data-testid'] ?? 'agent-session-view';
  if (!injected && (sessionId ?? null) === null) {
    return (
      <div className="flex min-h-0 flex-1 flex-col" data-testid={testId}>
        <SessionAbsent />
      </div>
    );
  }
  const actions: AgentSessionActions = {
    running: state.running,
    loading: state.loading,
    send: state.sendMessage,
    stop: state.stop,
  };
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid={testId}>
      {state.session !== null && <SessionMeta summary={state.session} running={state.running} />}
      {state.error !== null && (
        <div
          className="mb-2 shrink-0 break-all text-[13px] text-fail"
          data-testid="agent-session-error"
        >
          {state.error}
        </div>
      )}
      <AgentMessages messages={state.messages} loading={state.loading} running={state.running} />
      {composer?.(actions)}
    </div>
  );
}
