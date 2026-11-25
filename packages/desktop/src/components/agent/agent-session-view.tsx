import { useMemo, useState } from 'react';

import { StandardDrawer } from '@/components/standard/drawer';
import { Badge, Button } from '@/components/ui';
import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card';
import {
  useAgentSession,
  type AgentSessionSendInput,
  type UseAgentSessionState,
} from '@/hooks/use-agent-session';
import { cacheHitRate, formatPercent, formatTokens } from '@/lib/token';
import type { SessionPreambleInfo, SessionStats, SessionSummary } from '@/types/generated/bindings';

import { latestAgentFailure, type AgentFailure } from './agent-error';
import { type AskAnswerActions, AgentMessages } from './agent-messages';

/**
 * composer 插槽入参：运行态 + 动作面（插槽内容据此呈现发送 / 停止 / 补充说明）。
 * `steer` / `steering` 为 additive 加位——不接的消费点零影响；能否发补充说明由
 * 插槽自己决定（本组件不预判）。
 */
export interface AgentSessionActions {
  running: boolean;
  loading: boolean;
  send: (input: AgentSessionSendInput) => void;
  stop: () => void;
  /** 补充说明注入（运行中会话） */
  steer: (text: string) => void;
  /** 已提交、等 ack 回流（供插槽呈「排队中…」并禁发防连点） */
  steering: boolean;
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
  /** ask 应答动作面（可选注入位；与 composer 插槽互不前置——只读消费点可只接
   * 它，本组件原样下行，**不自动接线**） */
  askAnswer?: AskAnswerActions;
  'data-testid'?: string;
}

/**
 * 快照抽屉的静态指引（组件内常量）：与快照正文末段的冻结常量、与
 * `SessionPanelState.note` 槽位无关，只解释「冻结了、真身在哪、怎么生效」。
 */
const PREAMBLE_GUIDE =
  '本会话的系统提示词在会话开始时冻结，此后每轮逐字复用，不随文件变更改写；规则的真身是 AGENT.md。需要按最新规则执行时，直接要求 agent 重新读取 AGENT.md 即可。';

/** 会话系统提示词快照抽屉：只读呈现（零取数、零写面、不进对话流） */
function PreambleDrawer({ preamble }: { preamble: SessionPreambleInfo }): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const bytes = new TextEncoder().encode(preamble.content).length;
  return (
    <>
      <Button
        variant="link"
        size="xs"
        data-testid="session-meta-preamble"
        aria-label="系统提示词"
        onClick={() => setOpen(true)}
      >
        系统提示词
      </Button>
      <StandardDrawer
        title="系统提示词快照"
        data-testid="session-preamble-drawer"
        containerClassName="w-[720px] max-w-[85vw]"
        content={
          <div className="flex flex-col gap-3 text-[13px]">
            <div className="text-muted-foreground" data-testid="session-preamble-meta">
              拍存于 {new Date(preamble.savedAt).toLocaleString()} · {bytes} 字节
            </div>
            <div
              className="rounded-md bg-muted px-3 py-2 text-[12px] text-muted-foreground"
              data-testid="session-preamble-guide"
            >
              {PREAMBLE_GUIDE}
            </div>
            <pre
              className="m-0 font-mono text-[12px] whitespace-pre-wrap break-words select-text"
              data-testid="session-preamble-content"
            >
              {preamble.content}
            </pre>
          </div>
        }
        open={open}
        onOpenChange={setOpen}
      />
    </>
  );
}

/** token 用量卡片：悬停元信息行合计展开输入 / 输出 / 缓存读写与命中率。
 * 命中率 = 缓存读取 ÷ 输入（rig 口径：输入已含缓存读写），任一侧缺席即无定义。 */
function TokenCard({ stats }: { stats: SessionStats }): React.JSX.Element {
  const hitRate = cacheHitRate(stats.cachedInputTokens, stats.inputTokens);
  return (
    <div className="flex flex-col gap-2">
      <div className="font-medium text-popover-foreground">Token 用量</div>
      <div className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
        <span className="text-muted-foreground">输入</span>
        <span className="text-right font-mono tabular-nums" data-testid="session-token-input">
          {formatTokens(stats.inputTokens)}
        </span>
        <span className="text-muted-foreground">输出</span>
        <span className="text-right font-mono tabular-nums" data-testid="session-token-output">
          {formatTokens(stats.outputTokens)}
        </span>
      </div>
      <div className="border-t border-border" />
      <div className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
        <span className="text-muted-foreground">缓存命中率</span>
        <span
          className="text-right font-mono font-medium tabular-nums"
          data-testid="session-token-cache-hit-rate"
        >
          {formatPercent(hitRate)}
        </span>
        <span className="text-muted-foreground">缓存读取</span>
        <span className="text-right font-mono tabular-nums" data-testid="session-token-cache-read">
          {formatTokens(stats.cachedInputTokens)}
        </span>
        <span className="text-muted-foreground">缓存写入</span>
        <span className="text-right font-mono tabular-nums" data-testid="session-token-cache-write">
          {formatTokens(stats.cacheCreationInputTokens)}
        </span>
      </div>
    </div>
  );
}

/** 会话元信息区（会话 id / 运行中徽章 / 轮数 / tokens / 快照入口；自组件状态读取） */
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
        className="max-w-70 truncate font-mono"
        title={summary.row.id}
        data-testid="session-meta-id"
      >
        {summary.row.id}
      </span>
      <Badge variant={running ? 'default' : 'secondary'} data-testid="session-meta-status">
        {running ? '运行中' : '已收口'}
      </Badge>
      <span data-testid="session-meta-turns">对话 {summary.stats.turnCount} 轮</span>
      <HoverCard>
        <HoverCardTrigger
          delay={120}
          render={
            <button type="button" className="cursor-help" data-testid="session-meta-tokens">
              消耗token{' '}
              {formatTokens((summary.stats.inputTokens ?? 0) + (summary.stats.outputTokens ?? 0))}
            </button>
          }
        />
        <HoverCardContent className="w-60" data-testid="session-token-card">
          <TokenCard stats={summary.stats} />
        </HoverCardContent>
      </HoverCard>
      {summary.preamble !== null && <PreambleDrawer preamble={summary.preamble} />}
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

/** 最新轮失败横幅：友好文案 + 可展开的原始错误（诊断不丢） */
function AgentFailureBanner({ failure }: { failure: AgentFailure }): React.JSX.Element {
  return (
    <div
      className="mb-2 shrink-0 rounded-md bg-fail-bg px-3 py-2 text-[13px] text-fail"
      data-testid="agent-session-failure"
      role="alert"
    >
      <div className="font-medium" data-testid="agent-session-failure-title">
        {failure.title}
      </div>
      <div data-testid="agent-session-failure-hint">{failure.hint}</div>
      {failure.raw !== '' && (
        <details className="mt-1">
          <summary className="cursor-pointer text-xs text-muted-foreground">原始错误</summary>
          <div
            className="mt-1 break-all font-mono text-xs text-muted-foreground"
            data-testid="agent-session-failure-raw"
          >
            {failure.raw}
          </div>
        </details>
      )}
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
  const { root, sessionId, refreshKey, session, composer, askAnswer } = props;
  const injected = session !== undefined;
  const internal = useAgentSession({
    root: injected ? null : (root ?? null),
    sessionId: injected ? null : (sessionId ?? null),
    source: 'debug',
    refreshKey: injected ? undefined : refreshKey,
  });
  const state = session ?? internal;
  const failure = useMemo(
    () => latestAgentFailure(state.events, state.chain),
    [state.events, state.chain],
  );
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
    steer: state.steer,
    steering: state.steering,
  };
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid={testId}>
      {state.session !== null && <SessionMeta summary={state.session} running={state.running} />}
      {failure !== null && <AgentFailureBanner failure={failure} />}
      {state.error !== null && (
        <div
          className="mb-2 shrink-0 break-all text-[13px] text-fail"
          data-testid="agent-session-error"
        >
          {state.error}
        </div>
      )}
      <AgentMessages messages={state.messages} loading={state.loading} askAnswer={askAnswer} />
      {composer?.(actions)}
    </div>
  );
}
