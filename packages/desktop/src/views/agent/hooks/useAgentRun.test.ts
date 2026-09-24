import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentRunRecord } from '../../../types/dto';
import { useAgentRun } from './useAgentRun';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发；Channel mock 为可编程 class（捕获
// onmessage 回调，测试内手动喂事件）——真实 Tauri IPC 传输语义属库自带语义。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function textMessage(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text: `正文 ${seq}` }],
    parentToolUseId: null,
  };
}

const COMPLETED_RECORD: AgentRunRecord = {
  id: 1,
  prompt: '你好',
  cwd: 'C:\\demo\\beta',
  env: 'default',
  permissionMode: 'bypassPermissions',
  status: 'completed',
  startedAt: 1727000000000,
  finishedAt: 1727000001000,
  numTurns: 3,
  costUsd: 0.5,
  durationMs: 1234,
  sessionId: 's-1',
  error: null,
  source: 'debug',
  sourceRef: null,
  parentRunId: null,
};

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: ((event: unknown) => void) | null;
}

/** 最近一次创建的 Channel 实例（start 时 new）。 */
function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('start 未创建 Channel');
  return instance;
}

describe('useAgentRun：start 参数契约与实时事件累积', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    // 默认悬挂：start 后保持 running，个别用例经 mockResolvedValueOnce /
    // mockRejectedValueOnce / 显式 mockImplementation 覆盖
    invokeMock.mockImplementation(() => new Promise(() => {}));
    ChannelMock.instances.length = 0;
  });

  it('root 为 null 时 start 不发起 invoke（禁用语义）', () => {
    const { result } = renderHook(() => useAgentRun(null));

    act(() => {
      result.current.start({ prompt: '你好', env: 'default', permissionMode: 'bypassPermissions' });
    });

    expect(invokeMock).not.toHaveBeenCalled();
    expect(result.current.running).toBe(false);
  });

  it('start 以 camelCase IPC 键调用 invoke("agent_start")：onEvent 为 Channel 实例 + root/prompt/env/permissionMode', () => {
    const { result } = renderHook(() => useAgentRun('C:\\demo\\beta'));

    act(() => {
      result.current.start({ prompt: '你好', env: 'bare', permissionMode: 'acceptEdits' });
    });

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('agent_start', {
      onEvent: expect.any(ChannelMock),
      root: 'C:\\demo\\beta',
      prompt: '你好',
      env: 'bare',
      permissionMode: 'acceptEdits',
    });
  });

  it('发起后 running=true，Channel 逐事件回调使 events 按序累积', () => {
    invokeMock.mockImplementation(() => new Promise(() => {})); // 悬挂：保持 running
    const { result } = renderHook(() => useAgentRun('C:\\demo\\beta'));

    act(() => {
      result.current.start({ prompt: '你好', env: 'default', permissionMode: 'bypassPermissions' });
    });
    expect(result.current.running).toBe(true);
    expect(result.current.events).toEqual([]);

    act(() => {
      lastChannel().onmessage?.(runStarted(0));
    });
    act(() => {
      lastChannel().onmessage?.(textMessage(1));
    });

    expect(result.current.events).toEqual([runStarted(0), textMessage(1)]);
    expect(result.current.running).toBe(true);
  });

  it('invoke resolve 记录：result 置记录、running=false（completed 与 failed 两形态）', async () => {
    invokeMock.mockResolvedValueOnce(COMPLETED_RECORD);
    const { result } = renderHook(() => useAgentRun('C:\\demo\\beta'));

    act(() => {
      result.current.start({ prompt: '你好', env: 'default', permissionMode: 'bypassPermissions' });
    });
    await waitFor(() => expect(result.current.result).toEqual(COMPLETED_RECORD));
    expect(result.current.running).toBe(false);
    expect(result.current.error).toBeNull();

    // failed 终态同形态（D7：in-band 失败同样以记录 resolve）
    const failedRecord: AgentRunRecord = { ...COMPLETED_RECORD, status: 'failed', numTurns: 1 };
    invokeMock.mockResolvedValueOnce(failedRecord);
    act(() => {
      result.current.start({ prompt: '再来', env: 'default', permissionMode: 'bypassPermissions' });
    });
    await waitFor(() => expect(result.current.result).toEqual(failedRecord));
    expect(result.current.running).toBe(false);
  });

  it('invoke reject(string)：error 置串、running=false，且可再次 start', async () => {
    invokeMock.mockRejectedValueOnce('CLI 未找到');
    const { result } = renderHook(() => useAgentRun('C:\\demo\\beta'));

    act(() => {
      result.current.start({ prompt: '你好', env: 'default', permissionMode: 'bypassPermissions' });
    });
    await waitFor(() => expect(result.current.error).toBe('CLI 未找到'));
    expect(result.current.running).toBe(false);
    expect(result.current.result).toBeNull();

    // 可重试：错误如实呈现后再次 start 发起新 invoke
    invokeMock.mockResolvedValueOnce(COMPLETED_RECORD);
    act(() => {
      result.current.start({ prompt: '重试', env: 'default', permissionMode: 'bypassPermissions' });
    });
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(result.current.error).toBeNull();
  });

  it('事件洪峰（300 条）全部累积不丢', () => {
    invokeMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useAgentRun('C:\\demo\\beta'));

    act(() => {
      result.current.start({ prompt: '你好', env: 'default', permissionMode: 'bypassPermissions' });
    });
    act(() => {
      for (let seq = 0; seq < 300; seq += 1) {
        lastChannel().onmessage?.(textMessage(seq));
      }
    });

    expect(result.current.events).toHaveLength(300);
    expect(result.current.events.at(-1)?.seq).toBe(299);
  });

  it('再次 start 时新运行重置事件与错误（running 防并发忽略重复 start）', () => {
    invokeMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useAgentRun('C:\\demo\\beta'));

    act(() => {
      result.current.start({
        prompt: '第一跑',
        env: 'default',
        permissionMode: 'bypassPermissions',
      });
    });
    act(() => {
      lastChannel().onmessage?.(runStarted(0));
    });
    // 运行中重复 start 被忽略（不产生新 invoke / 新 Channel）
    act(() => {
      result.current.start({
        prompt: '第二跑',
        env: 'default',
        permissionMode: 'bypassPermissions',
      });
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(ChannelMock.instances).toHaveLength(1);
    expect(result.current.events).toEqual([runStarted(0)]);
  });
});
