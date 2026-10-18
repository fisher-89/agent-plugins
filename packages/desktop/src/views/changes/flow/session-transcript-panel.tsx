import { AgentSessionView, type AgentSessionViewProps } from '@/components/agent';

import type { RoleSessionRef } from './types';

interface SessionTranscriptPanelProps {
  root: string | null;
  roleRef: RoleSessionRef | null;
  /** 会话状态重评估信号（会话事件通知并入统一视图重查 tick；组件据其重评估
   * 是否挂流——delta 不经通知重查，实时面走会话订阅） */
  refreshKey?: number;
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
 * role 会话转录面板（只读消费点）：会话 id 直查 + 会话级组件自持形态渲染
 * （不接 composer 插槽，无发送 / 停止入口）。`roleRef.sessionId` 缺席呈步面
 * 空态（「此步无会话记录」），`roleRef` 缺席呈「当前选中无关联会话」。
 */
export function SessionTranscriptPanel({
  root,
  roleRef,
  refreshKey,
}: SessionTranscriptPanelProps): React.JSX.Element {
  if (!roleRef) {
    return <SessionSlotEmpty text="（当前选中无关联会话）" />;
  }
  if (roleRef.sessionId === null) {
    return <SessionSlotEmpty text="此步无会话记录" />;
  }
  // 自持形态入参：本面板的 root / sessionId / 重评估键即视图 props（只读，无插槽）
  const viewProps: AgentSessionViewProps = {
    root,
    sessionId: roleRef.sessionId,
    refreshKey,
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
