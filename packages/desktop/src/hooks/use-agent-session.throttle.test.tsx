import { useChat } from '@ai-sdk/react';
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { useAgentSession } from './use-agent-session';

// ---------------------------------------------------------------------------
// 节流接线回归护栏：只观测「useChat 以哪个 throttle 窗订阅 messages」，不复制
// 库的 leading/trailing 语义做脆弱时序断言。@ai-sdk/react 保留真实实现
// （spy），invoke 进程边界全部回 null（无重放、无订阅）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));
vi.mock(import('@ai-sdk/react'), { spy: true });

describe('useAgentSession：流式 messages 通知节流', () => {
  it('useChat 以 100ms 节流窗订阅 messages', () => {
    invokeMock.mockResolvedValue(null);
    renderHook(() =>
      useAgentSession({ root: 'C:\\demo\\alpha', sessionId: 'ses-1', source: 'test' }),
    );

    const call = vi.mocked(useChat).mock.calls.at(-1);
    expect(call?.[0]).toMatchObject({ throttle: 100 });
  });
});
