import { useCallback, useEffect, useRef, useState } from 'react';

import { useAgentSession, type UseAgentSessionState } from '../../../hooks/use-agent-session';
import { buildExplorePrompt } from '../../../lib/explore-stance';
import { ipc } from '../../../lib/ipc';
import type { AgentPermissionMode, ExploreRecord } from '../../../types/dto';

/** explore 会话来源受控字符串（与 store 侧来源归属口径一致） */
const EXPLORE_SOURCE = 'explore';

/** auto-send 档位（与 composer 缺省档同源——components/agent AgentInput 缺省
 * 'bypassPermissions'） */
const AUTO_SEND_PERMISSION_MODE: AgentPermissionMode = 'bypassPermissions';

/** 单次发送入参（与调试页表单语义一致：cwd 隐含 workspace root） */
export interface ExploreSendInput {
  prompt: string;
  permissionMode: AgentPermissionMode;
}

/**
 * 存量记录（session_id 空缺——v3 升级行）经 ensure 补齐归属会话后承接其 id：
 * 建档新记录恒带预建空会话 id，本路只补历史缺口；补齐失败静默（send 按未就绪
 * 不发，下次挂载再试）。
 */
function useEnsuredSessionId(
  root: string | null,
  recordName: string | null,
  recordedSessionId: string | null,
): string | null {
  const [ensuredId, setEnsuredId] = useState<string | null>(null);
  useEffect(() => {
    if (root === null || recordName === null || recordedSessionId !== null) {
      setEnsuredId(null);
      return;
    }
    let cancelled = false;
    ipc
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
  return recordedSessionId ?? ensuredId;
}

/**
 * auto-send 闸门：建档导航携带的 autoPrompt 在「重放装载完成（!loading）&&
 * 会话链为空 && 记录 / 会话 id 就绪」且未消费时自动发出首轮（stance 前导 +
 * 需求）。per-mount 一次性消费 ref——back/forward 复现 location state 时新
 * mount 重新武装，首轮失败 chain 仍 0 即构成免费重试；发出后 chain > 0 闸门
 * 自然拒发（send 内建 loading / 未就绪守卫同样生效）。
 *
 * 装载观测武装（armedRef）：首渲染 `loading` 恒为滞后 false（重放 effect 同
 * 提交稍后才置真），若直接判定「!loading 即装载完成」会在装载起点抢跑——
 * 发出的轮行随即被 applyReplay 重建覆盖（chain 丢失）。故按会话 id 记「已
 * 观测到该会话 loading 真值」，武装后方可在 !loading 时判定装载完成；会话
 * 变更（record 切换）重新武装，逐会话走完整观测序列。
 */
function useAutoSend(
  session: UseAgentSessionState,
  record: ExploreRecord | null,
  sessionId: string | null,
  autoPrompt: string | null,
  send: (input: ExploreSendInput) => void,
): void {
  const autoSentRef = useRef(false);
  const armedRef = useRef<string | null>(null);
  useEffect(() => {
    if (sessionId === null) {
      armedRef.current = null;
      return;
    }
    if (armedRef.current !== sessionId) {
      // 未武装：仅在本会话 loading 真值经渲染到达后武装（真值必然经历至少
      // 一次重渲染，act/真实调度下均可观测）
      if (session.loading) armedRef.current = sessionId;
      return;
    }
    if (
      autoPrompt !== null &&
      !autoSentRef.current &&
      !session.loading &&
      session.chain.length === 0 &&
      record !== null
    ) {
      autoSentRef.current = true;
      send({ prompt: autoPrompt, permissionMode: AUTO_SEND_PERMISSION_MODE });
    }
  }, [autoPrompt, session, record, sessionId, send]);
}

/** explore 会话状态面（来源侧组装：消息 / 轮行 / 库态运行标记经 `session` 透出） */
export interface ExploreSessionState {
  session: UseAgentSessionState;
  /** 记录回写的会话 id（会话寻址键；null = 尚未落位） */
  sessionId: string | null;
  send: (input: ExploreSendInput) => void;
  stop: () => void;
}

export function useExploreSession(
  root: string | null,
  record: ExploreRecord | null,
  autoPrompt: string | null = null,
): ExploreSessionState {
  const recordName = record?.name ?? null;
  const recordedSessionId = record?.sessionId ?? null;
  const sessionId = useEnsuredSessionId(root, recordName, recordedSessionId);
  const session = useAgentSession({
    source: EXPLORE_SOURCE,
    sessionId,
    root,
  });

  const send = useCallback(
    (input: ExploreSendInput) => {
      // 未就绪不发：记录缺席 / 会话 id 尚未落位 / 重放在途（stance 判定需
      // 轮数真相，装载完成前不发起）
      if (record === null || sessionId === null || session.loading) return;
      // 首轮（会话无任何轮统计行）拼 stance 前导（占位 / 已定名两形由 stance
      // 模块内按记录名前缀判别）；续话裸发送
      const prompt =
        session.chain.length === 0 ? buildExplorePrompt(input.prompt, record.name) : input.prompt;
      session.sendMessage({
        prompt,
        permissionMode: input.permissionMode,
      });
    },
    [session, record, sessionId],
  );

  useAutoSend(session, record, sessionId, autoPrompt, send);

  return { session, sessionId, send, stop: session.stop };
}
