import { Channel, invoke } from '@tauri-apps/api/core';
import { useCallback, useRef, useState } from 'react';

import type { AgentEvent, AgentRunRecord } from '../../../types/dto';
import type { AgentStartInput } from '../components/AgentRunForm';

export interface AgentRunState {
  /** 实时事件累积（Channel 逐事件推送） */
  events: AgentEvent[];
  /** 是否运行中（invoke 未 resolve / reject） */
  running: boolean;
  /** 启动阶段失败（CLI 缺失 / spawn 失败 / store 失败）；运行内失败走 result 终态 */
  error: string | null;
  /** run 结束时的最终记录（含 failed 终态） */
  result: AgentRunRecord | null;
  /** 发起运行；root 为 null 或已有运行进行中时忽略 */
  start: (input: AgentStartInput) => void;
}

/**
 * 实时运行 hook：`new Channel<AgentEvent>()` 订阅 + invoke("agent_start")，
 * 事件经 onmessage 逐条累积；该订阅属执行流通道（命令作用域实时流），不属
 * 「刷新取数模型」禁止的轮询取数。组件不直接 invoke。运行中重复 start 忽略
 * （ref 防并发）；新运行重置事件 / 错误 / 终态。
 */
export function useAgentRun(root: string | null): AgentRunState {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<AgentRunRecord | null>(null);
  const runningRef = useRef(false);

  const start = useCallback(
    (input: AgentStartInput) => {
      if (root === null || runningRef.current) return;
      runningRef.current = true;
      setRunning(true);
      setError(null);
      setResult(null);
      setEvents([]);
      const channel = new Channel<AgentEvent>();
      channel.onmessage = (event) => {
        setEvents((prev) => [...prev, event]);
      };
      invoke<AgentRunRecord>('agent_start', {
        onEvent: channel,
        root,
        prompt: input.prompt,
        env: input.env,
        permissionMode: input.permissionMode,
      })
        .then((record) => {
          setResult(record);
        })
        .catch((err: unknown) => {
          setError(String(err));
        })
        .finally(() => {
          runningRef.current = false;
          setRunning(false);
        });
    },
    [root],
  );

  return { events, running, error, result, start };
}
