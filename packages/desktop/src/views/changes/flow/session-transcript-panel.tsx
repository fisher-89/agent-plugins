import {
  type AskAnswerActions,
  AgentInput,
  AgentSessionView,
  type AgentSessionViewProps,
} from '@/components/agent';

import type { RoleSessionRef } from './types';

interface SessionTranscriptPanelProps {
  root: string | null;
  roleRef: RoleSessionRef | null;
  /** 会话状态重评估信号（会话事件通知并入统一视图重查 tick；组件据其重评估
   * 是否挂流——delta 不经通知重查，实时面走会话订阅） */
  refreshKey?: number;
  /** ask 应答动作面（与 composer 插槽互不前置） */
  askAnswer?: AskAnswerActions;
}

/** 槽位缺席空态（不虚构会话、零取数零订阅） */
function SessionSlotEmpty({ text }: { text: string }): React.JSX.Element {
  return (
    <div className="text-[13px] text-muted-foreground" data-testid="drawer-session-empty">
      {text}
    </div>
  );
}

/**
 * role 会话转录面板（接 composer 插槽）：会话 id 直查 + 会话级组件自持形态
 * 渲染，插槽呈补充说明输入 + 停止；不呈 permission-mode 档位与新会话入口，
 * 可用性不预判（轮是否在跑由后端 `Err` 说话）。`roleRef.sessionId` 缺席呈步面
 * 空态（「此步无会话记录」，不渲染 composer），`roleRef` 缺席呈「当前选中无
 * 关联会话」。
 */
export function SessionTranscriptPanel({
  root,
  roleRef,
  refreshKey,
  askAnswer,
}: SessionTranscriptPanelProps): React.JSX.Element {
  if (!roleRef) {
    return <SessionSlotEmpty text="（当前选中无关联会话）" />;
  }
  if (roleRef.sessionId === null) {
    return <SessionSlotEmpty text="此步无会话记录" />;
  }
  // 自持形态入参：本面板的 root / sessionId / 重评估键即视图 props
  const viewProps: AgentSessionViewProps = {
    root,
    sessionId: roleRef.sessionId,
    refreshKey,
    // 补充说明输入：模型在下一个请求边界看到该文本（运行中有效，否则后端 Err）
    composer: (actions) => (
      <AgentInput
        idPrefix="step-session"
        placeholder="给这一步补一句话（模型在下一个请求边界看到）"
        running={actions.running}
        hidePermissionMode
        pending={actions.steering}
        onSend={({ text }) => actions.steer(text)}
        onStop={actions.stop}
      />
    ),
    ...(askAnswer === undefined ? {} : { askAnswer }),
    'data-testid': 'session-transcript',
  };
  return (
    <section className="flex h-full min-h-0 flex-col" data-testid="session-transcript-panel">
      <div className="flex min-h-0 flex-1 flex-col" data-transcript-role={roleRef.role}>
        <AgentSessionView {...viewProps} />
      </div>
    </section>
  );
}
