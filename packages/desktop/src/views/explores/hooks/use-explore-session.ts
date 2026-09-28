/**
 * explore 会话 hook（来源侧）：内部改走统一会话基建 use-agent-chat——
 * 重放装载 / 链参数组装 / 运行门闩 / 停止接线全部由基建承担，本 hook 只
 * 保留 explore 侧语义：stance 前导拼接（buildExplorePrompt）与来源三元组
 * （source="explore"、sourceRef=记录 id 十进制串）。对外状态语义（双恢复 /
 * 续话 / running / error）与迁移前一致，另增停止透传。
 */

import { useCallback } from 'react';

import { useAgentChat } from '../../../hooks/use-agent-chat';
import { buildExplorePrompt } from '../../../lib/explore-stance';
import type { AgentPermissionMode, ExploreRecord } from '../../../types/dto';

/** explore 会话来源受控字符串（与 store 侧 RunProvenance 口径一致） */
const EXPLORE_SOURCE = 'explore';

/** 单次发送入参（与调试页表单语义一致：cwd 隐含 workspace root） */
export interface ExploreSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
}

/**
 * 会话链 hook（对话区数据面）：mount / 记录变更经基建重放装载（链还原 +
 * 逐 run 事件重放，重开双恢复的对话半边）；send 拼 stance 后委托基建，以
 * 链尾（非 running）run 的 sessionId 续话、parentRunId 取链尾 id；run 终态
 * record 部件回流后链尾推进（下次续话以新链尾为准）。运行中防重复发送，
 * record 为 null（未选定记录）不发送。
 */
export function useExploreSession(root: string | null, record: ExploreRecord | null) {
  const session = useAgentChat({
    source: EXPLORE_SOURCE,
    sourceRef: record !== null ? String(record.id) : null,
    root,
  });

  /** 发送一条消息：拼 stance、委托基建（链参数组装在基建内收口）；record 为 null（未选定记录）不发送 */
  const send = useCallback(
    (input: ExploreSendInput) => {
      if (record === null) return;
      session.sendMessage({
        prompt: buildExplorePrompt(input.prompt),
        permissionMode: input.permissionMode,
      });
    },
    [session, record],
  );

  return {
    messages: session.messages,
    /** 会话链镜像（发起序 run 记录；迁移基线沿用，供既有断言与调试） */
    chain: session.chain,
    /** 事件扁平镜像（链序逐 run 拼接；迁移基线沿用） */
    events: session.events,
    loading: session.loading,
    running: session.running,
    error: session.error,
    send,
    stop: session.stop,
  };
}
