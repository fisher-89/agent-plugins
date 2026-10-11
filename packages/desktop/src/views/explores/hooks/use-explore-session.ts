import { useCallback } from 'react';

import { useAgentChat } from '../../../hooks/use-agent-chat';
import { buildExplorePrompt } from '../../../lib/explore-stance';
import type { AgentPermissionMode, ExploreRecord } from '../../../types/dto';

/** explore 会话来源受控字符串（与 store 侧来源归属口径一致） */
const EXPLORE_SOURCE = 'explore';

/** 单次发送入参（与调试页表单语义一致：cwd 隐含 workspace root） */
export interface ExploreSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
}

export function useExploreSession(root: string | null, record: ExploreRecord | null) {
  const session = useAgentChat({
    source: EXPLORE_SOURCE,
    sourceRef: record !== null ? String(record.id) : null,
    root,
  });

  const send = useCallback(
    (input: ExploreSendInput) => {
      if (record === null) return;
      const prompt =
        session.session === null ? buildExplorePrompt(input.prompt, record.name) : input.prompt;
      session.sendMessage({
        prompt,
        permissionMode: input.permissionMode,
      });
    },
    [session, record],
  );

  return {
    messages: session.messages,
    /** 轮统计行镜像（发起序轮行；迁移基线沿用，供既有断言与调试） */
    chain: session.chain,
    /** 密封事件扁平镜像（delta 仅实时流可见不入镜像；迁移基线沿用） */
    events: session.events,
    loading: session.loading,
    running: session.running,
    error: session.error,
    send,
    stop: session.stop,
  };
}
