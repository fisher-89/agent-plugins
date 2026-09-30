import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentInstanceRecord } from '../../../types/generated/bindings';
import { useAgentOptions } from './use-agent-options';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（list_agent_instances 固定 fixture，可切换
// resolve / reject 并记录调用序列）；取数收口 hook（挂载一次 + defaultId 派生）。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

/** agent 实例 fixture（serde camelCase 线格式；providerId 对选择器面无关可空）。 */
function agentInstance(
  id: number,
  name: string,
  engine: AgentInstanceRecord['engine'],
  isDefault = false,
): AgentInstanceRecord {
  return { id, name, engine, providerId: null, isDefault };
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

describe('useAgentOptions：选择器取数（AC-8 前置）', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('挂载 invoke list_agent_instances 恰一次；defaultId = is_default 记录 id；instances 态回填、loading 收敛', async () => {
    invokeMock.mockResolvedValue([
      agentInstance(3, 'cli-a', 'cli'),
      agentInstance(7, 'sdk-b', 'sdk', true),
    ]);
    const { result } = renderHook(() => useAgentOptions());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invokeMock.mock.calls.map(([name]) => name)).toEqual(['list_agent_instances']);
    expect(result.current.instances).toHaveLength(2);
    expect(result.current.instances[0]).toEqual(agentInstance(3, 'cli-a', 'cli'));
    expect(result.current.defaultId).toBe(7);
    expect(result.current.loading).toBe(false);
  });

  it('无默认记录 → defaultId null（表单保持缺省语义）', async () => {
    invokeMock.mockResolvedValue([
      agentInstance(3, 'cli-a', 'cli'),
      agentInstance(5, 'sdk-b', 'sdk'),
    ]);
    const { result } = renderHook(() => useAgentOptions());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.instances).toHaveLength(2);
    expect(result.current.defaultId).toBeNull();
  });

  it('空清单 → instances [] + defaultId null（不崩）', async () => {
    invokeMock.mockResolvedValue([]);
    const { result } = renderHook(() => useAgentOptions());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.instances).toEqual([]);
    expect(result.current.defaultId).toBeNull();
  });

  it('无轮询：取数链稳定判据（连续轮询调用数不再增长）', async () => {
    // 每个响应经 setTimeout(0) 宏任务逐拍解析（沿 use-workspaces 稳定判据口径）
    invokeMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          setTimeout(() => resolve([agentInstance(7, 'sdk-b', 'sdk', true)]), 0);
        }),
    );
    const { result } = renderHook(() => useAgentOptions());

    // 等待取数链稳定：连续两次轮询调用数不再增长
    let previous = -1;
    await waitFor(
      () => {
        const current = invokeMock.mock.calls.length;
        if (current !== previous) {
          previous = current;
          throw new Error('取数链尚未稳定');
        }
      },
      { timeout: 3000, interval: 50 },
    );

    expect(result.current.defaultId).toBe(7);
    expect(countOf('list_agent_instances')).toBe(1);
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('清单加载失败静默为空清单：instances []、loading 收敛、不抛未捕获异常（发起面保持可用）', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    const { result } = renderHook(() => useAgentOptions());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.instances).toEqual([]);
    expect(result.current.defaultId).toBeNull();
  });

  it('卸载取消：清单在卸载后才返回时不置状态、不再取数', async () => {
    let resolveLoad: (records: AgentInstanceRecord[]) => void = () => {};
    invokeMock.mockImplementation(
      () =>
        new Promise<AgentInstanceRecord[]>((resolve) => {
          resolveLoad = resolve;
        }),
    );
    const { unmount } = renderHook(() => useAgentOptions());
    unmount();

    await act(async () => {
      resolveLoad([agentInstance(7, 'sdk-b', 'sdk', true)]);
    });

    expect(countOf('list_agent_instances')).toBe(1);
  });
});
