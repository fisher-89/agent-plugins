import { useCallback, useEffect, useState } from 'react';

import { useAgentChat } from '../../../hooks/use-agent-chat';
import { buildExplorePrompt } from '../../../lib/explore-stance';
import type { AgentPermissionMode, ExploreRecord } from '../../../types/dto';
import { commands } from '../../../types/generated/bindings';

/** explore 会话来源受控字符串（与 store 侧来源归属口径一致） */
const EXPLORE_SOURCE = 'explore';

/** 单次发送入参（与调试页表单语义一致：cwd 隐含 workspace root） */
export interface ExploreSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
}

export function useExploreSession(root: string | null, record: ExploreRecord | null) {
  // 存量记录（session_id 空缺——v3 升级行）经 ensure 补齐归属会话后承接其 id：
  // 建档新记录恒带预建空会话 id，本路只补历史缺口
  const [ensuredId, setEnsuredId] = useState<string | null>(null);
  const recordName = record?.name ?? null;
  const recordedSessionId = record?.sessionId ?? null;

  useEffect(() => {
    if (root === null || recordName === null || recordedSessionId !== null) {
      setEnsuredId(null);
      return;
    }
    let cancelled = false;
    commands
      .ensureExploreSession(root, recordName)
      .then((updated) => {
        if (!cancelled) setEnsuredId(updated.sessionId);
      })
      .catch(() => {
        // 静默：会话未就绪，send 按未就绪不发（下次挂载再试）
      });
    return () => {
      cancelled = true;
    };
  }, [root, recordName, recordedSessionId]);

  const sessionId = recordedSessionId ?? ensuredId;
  const session = useAgentChat({
    source: EXPLORE_SOURCE,
    sessionId,
    root,
  });

  const send = useCallback(
    (input: ExploreSendInput) => {
      // 未就绪不发：记录缺席 / 会话 id 尚未落位 / 重放在途（stance 判定需
      // 轮数真相，装载完成前不发起）
      if (record === null || sessionId === null || session.loading) return;
      // 首轮（会话无任何轮统计行）拼 stance 前导；续话裸发送
      const prompt =
        session.chain.length === 0 ? buildExplorePrompt(input.prompt, record.name) : input.prompt;
      session.sendMessage({
        prompt,
        permissionMode: input.permissionMode,
      });
    },
    [session, record, sessionId],
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
