import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { CodeStatsReport } from '../../../types/generated/bindings';
import { useCodeStats } from './use-code-stats';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// ---------------------------------------------------------------------------
// fixture（对齐生成绑定 CodeStatsReport serde camelCase；share 为 `number | null`
// 出线口径，此处按有值形态构造）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';

function makeReport(overrides: Partial<CodeStatsReport> = {}): CodeStatsReport {
  return {
    totals: { files: 12, code: 340, comments: 40, blanks: 20 },
    languages: [
      { name: 'Rust', files: 6, code: 220, comments: 30, blanks: 10, share: 64.7 },
      { name: 'TypeScript', files: 6, code: 120, comments: 10, blanks: 10, share: 35.3 },
    ],
    tree: [
      {
        kind: 'dir',
        node: {
          name: 'src',
          path: 'src',
          files: 9,
          code: 300,
          comments: 35,
          blanks: 18,
          children: [],
        },
      },
    ],
    ...overrides,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useCodeStats：显式刷新取数纪律（AC-4/AC-6）', () => {
  it('有效 root 挂载：以 (root, depth=5) 发起 code_stats 恰一次，report 呈现返回 DTO，loading true→false', async () => {
    let resolveLoad!: (value: CodeStatsReport) => void;
    const seeded = makeReport();
    invokeMock.mockImplementation(() => {
      return new Promise<CodeStatsReport>((resolve) => {
        resolveLoad = resolve;
      });
    });

    const { result } = renderHook(() => useCodeStats(ROOT));
    expect(result.current.loading).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('code_stats', { root: ROOT, depth: 5 });

    await act(async () => {
      resolveLoad(seeded);
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.report).toEqual(seeded);
    expect(result.current.error).toBeNull();
  });

  it('refresh() 显式调用：同 root 同 depth 恰再发一次', async () => {
    invokeMock.mockResolvedValue(makeReport());
    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: ROOT, depth: 5 });
  });

  it('setDepth(N)：恰以 depth=N 再发一次（自默认 5 起），depth 态同步更新', async () => {
    invokeMock.mockResolvedValue(makeReport());
    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.setDepth(8);
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: ROOT, depth: 8 });
    expect(result.current.depth).toBe(8);
  });

  it('setDepth 取值域端点 1 与 10：各以对应 depth 恰发起一次', async () => {
    invokeMock.mockResolvedValue(makeReport());
    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.setDepth(1);
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: ROOT, depth: 1 });

    act(() => {
      result.current.setDepth(10);
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(3));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: ROOT, depth: 10 });
  });

  it('连续 setDepth 5→3→7：三轮各以对应 depth 发起，最终 report 为 depth=7 轮结果（在途旧轮取消抑制）', async () => {
    invokeMock.mockImplementation((_command: string, params?: { depth?: number }) =>
      Promise.resolve(
        makeReport({
          totals: { files: 1, code: (params?.depth ?? 5) * 1000, comments: 0, blanks: 0 },
        }),
      ),
    );
    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.report?.totals.code).toBe(5000));

    // depth=3 轮在途时即切 depth=7：旧轮取消，其结果不落态
    act(() => {
      result.current.setDepth(3);
    });
    act(() => {
      result.current.setDepth(7);
    });
    await waitFor(() => expect(result.current.report?.totals.code).toBe(7000));
    await act(async () => {});
    expect(result.current.report?.totals.code).toBe(7000);

    const depths = invokeMock.mock.calls.map(([, params]) => (params as { depth: number }).depth);
    expect(depths).toEqual([5, 3, 7]);
  });

  it('取数完成后静置（推进定时器、无任何交互）：调用数不增长（无轮询的行为化核对）', async () => {
    invokeMock.mockResolvedValue(makeReport());
    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock.mock.calls.length).toBe(1);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    expect(invokeMock.mock.calls.length).toBe(1);
  });

  it('codeStats reject（首取）：error 含错误串、loading 复位、report 保持 null', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));

    const { result } = renderHook(() => useCodeStats(ROOT));

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.loading).toBe(false);
    expect(result.current.report).toBeNull();
  });

  it('成功后 refresh() 再 reject：error 置位、report 保持上次成功值不崩（不污染惯例）', async () => {
    const seeded = makeReport();
    invokeMock.mockResolvedValueOnce(seeded).mockRejectedValueOnce(new Error('再次解析失败'));

    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.report).toEqual(seeded));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toContain('再次解析失败'));
    expect(result.current.report).toEqual(seeded);
    expect(result.current.loading).toBe(false);
  });
});

describe('useCodeStats：root 切换过渡与归属（AC-6）', () => {
  it('root A→B：以新 root 重取，B 结果到达后 report 归属 B', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) =>
      Promise.resolve(
        makeReport({
          totals: {
            files: 1,
            code: params?.root === 'C:\\demo\\b' ? 200 : 100,
            comments: 0,
            blanks: 0,
          },
        }),
      ),
    );

    const { result, rerender } = renderHook(({ root }: { root: string }) => useCodeStats(root), {
      initialProps: { root: 'C:\\demo\\a' },
    });
    await waitFor(() => expect(result.current.report?.totals.code).toBe(100));

    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() => expect(result.current.report?.totals.code).toBe(200));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: 'C:\\demo\\b', depth: 5 });
  });

  it('切换过渡轮（B 在途）：report 为 null，旧根报告不呈现（数据归属 root 标记抑制）', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return Promise.resolve(makeReport());
      }
      return new Promise<CodeStatsReport>(() => {});
    });

    const { result, rerender } = renderHook(({ root }: { root: string }) => useCodeStats(root), {
      initialProps: { root: 'C:\\demo\\a' },
    });
    await waitFor(() => expect(result.current.report).not.toBeNull());

    rerender({ root: 'C:\\demo\\b' });
    await act(async () => {});

    expect(result.current.report).toBeNull();
  });

  it('旧 root 迟到 resolve：不覆盖新态（cancelled 抑制）', async () => {
    let resolveA!: (value: CodeStatsReport) => void;
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return new Promise<CodeStatsReport>((resolve) => {
          resolveA = resolve;
        });
      }
      return Promise.resolve(
        makeReport({ totals: { files: 1, code: 200, comments: 0, blanks: 0 } }),
      );
    });

    const { result, rerender } = renderHook(({ root }: { root: string }) => useCodeStats(root), {
      initialProps: { root: 'C:\\demo\\a' },
    });
    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() => expect(result.current.report?.totals.code).toBe(200));

    await act(async () => {
      resolveA(makeReport());
    });
    expect(result.current.report?.totals.code).toBe(200);
  });

  it('旧 root 迟到 reject：不置错误、不污染新态', async () => {
    let rejectA!: (err: unknown) => void;
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return new Promise<CodeStatsReport>((_resolve, reject) => {
          rejectA = reject;
        });
      }
      return Promise.resolve(
        makeReport({ totals: { files: 1, code: 200, comments: 0, blanks: 0 } }),
      );
    });

    const { result, rerender } = renderHook(({ root }: { root: string }) => useCodeStats(root), {
      initialProps: { root: 'C:\\demo\\a' },
    });
    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() => expect(result.current.report).not.toBeNull());

    await act(async () => {
      rejectA(new Error('迟到的解析失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.report?.totals.code).toBe(200);
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-2 回归锁定）：裸 invoke → commands.codeStats typed 绑定
// 机械替换后，命令名与参数逐字不变（生成绑定底层同模块 invoke，mock 机制切换
// 后依旧生效）。
// ---------------------------------------------------------------------------

describe('useCodeStats：生成绑定调用面', () => {
  it('经 commands.codeStats typed 入口发起：命令名与参数 (root, depth) 逐字不变', async () => {
    invokeMock.mockResolvedValue(makeReport());

    const { result } = renderHook(() => useCodeStats(ROOT));
    await waitFor(() => expect(result.current.report).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledWith('code_stats', { root: ROOT, depth: 5 });

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: ROOT, depth: 5 });
    expect(invokeMock.mock.calls.every(([name]) => name === 'code_stats')).toBe(true);
  });
});
