import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { driftCount, driftReports, resetDrift } from '../../../lib/ipc-drift';
import type { WorkspaceConfig, WorkspaceConfigReport } from '../../../types/generated/bindings';
import { useWorkspaceConfig } from './use-workspace-config';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// ---------------------------------------------------------------------------
// fixture（对齐生成绑定 WorkspaceConfigReport serde camelCase；裸 f64 出线
// `number | null` 口径，此处按有值形态构造）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\alpha';

function makeConfig(overrides: Partial<WorkspaceConfig> = {}): WorkspaceConfig {
  return {
    $schema: null,
    schema: 'spec-driven',
    context: null,
    rules: null,
    staticAnalysis: null,
    tests: [],
    writeProtection: null,
    extra: [],
    ...overrides,
  };
}

function makeReport(overrides: Partial<WorkspaceConfigReport> = {}): WorkspaceConfigReport {
  return {
    config: makeConfig(),
    diagnostics: [],
    ...overrides,
  };
}

/** 以 context 承载归属 root 的报告（切换用例区分新旧根数据用）。 */
function reportForRoot(root: string): WorkspaceConfigReport {
  return makeReport({ config: makeConfig({ context: root }) });
}

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useWorkspaceConfig：显式刷新纪律（AC-6）', () => {
  it('挂载以 { root } 发起 workspace_config 恰一次，data 呈现返回报告，loading true→false', async () => {
    let resolveLoad!: (value: WorkspaceConfigReport) => void;
    const seeded = makeReport();
    invokeMock.mockImplementation(() => {
      return new Promise<WorkspaceConfigReport>((resolve) => {
        resolveLoad = resolve;
      });
    });

    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    expect(result.current.loading).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('workspace_config', { root: ROOT });

    await act(async () => {
      resolveLoad(seeded);
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.data).toEqual(seeded);
    expect(result.current.error).toBeNull();
  });

  it('refresh() 同 root 恰再发一次', async () => {
    invokeMock.mockResolvedValue(makeReport());
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('workspace_config', { root: ROOT });
  });

  it('取数完成后静置（推进定时器、无任何交互）：调用数不增长（无轮询的行为化核对）', async () => {
    invokeMock.mockResolvedValue(makeReport());
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invokeMock.mock.calls.length).toBe(1);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
    expect(invokeMock.mock.calls.length).toBe(1);
  });

  it('首取 reject：error 含错误串、loading 复位、data 保持 null', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));

    const { result } = renderHook(() => useWorkspaceConfig(ROOT));

    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.error).toContain('IPC 断开');
    expect(result.current.loading).toBe(false);
    expect(result.current.data).toBeNull();
  });

  it('成功后 refresh() 再 reject：error 置位、data 保持上次成功值不崩（不污染惯例）', async () => {
    const seeded = makeReport();
    invokeMock.mockResolvedValueOnce(seeded).mockRejectedValueOnce(new Error('再次解析失败'));

    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).toEqual(seeded));

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toContain('再次解析失败'));
    expect(result.current.data).toEqual(seeded);
    expect(result.current.loading).toBe(false);
  });
});

describe('useWorkspaceConfig：root 切换与归属（AC-6）', () => {
  it('root A→B：以新根重取，B 结果到达后 data 归属 B', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) =>
      Promise.resolve(reportForRoot(params?.root ?? '')),
    );

    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useWorkspaceConfig(root),
      {
        initialProps: { root: 'C:\\demo\\a' },
      },
    );
    await waitFor(() => expect(result.current.data?.config.context).toBe('C:\\demo\\a'));

    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() => expect(result.current.data?.config.context).toBe('C:\\demo\\b'));
    expect(invokeMock).toHaveBeenLastCalledWith('workspace_config', { root: 'C:\\demo\\b' });
  });

  it('切换过渡轮（B 在途）：data 为 null，旧根报告不呈现（归属 root 标记抑制）', async () => {
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return Promise.resolve(reportForRoot('C:\\demo\\a'));
      }
      return new Promise<WorkspaceConfigReport>(() => {});
    });

    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useWorkspaceConfig(root),
      {
        initialProps: { root: 'C:\\demo\\a' },
      },
    );
    await waitFor(() => expect(result.current.data).not.toBeNull());

    rerender({ root: 'C:\\demo\\b' });
    await act(async () => {});

    expect(result.current.data).toBeNull();
  });

  it('旧根迟到 resolve：不覆盖新态（cancelled 抑制）', async () => {
    let resolveA!: (value: WorkspaceConfigReport) => void;
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return new Promise<WorkspaceConfigReport>((resolve) => {
          resolveA = resolve;
        });
      }
      return Promise.resolve(reportForRoot('C:\\demo\\b'));
    });

    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useWorkspaceConfig(root),
      {
        initialProps: { root: 'C:\\demo\\a' },
      },
    );
    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() => expect(result.current.data?.config.context).toBe('C:\\demo\\b'));

    await act(async () => {
      resolveA(reportForRoot('C:\\demo\\a'));
    });
    expect(result.current.data?.config.context).toBe('C:\\demo\\b');
  });

  it('旧根迟到 reject：不置错误、不污染新态', async () => {
    let rejectA!: (err: unknown) => void;
    invokeMock.mockImplementation((_command: string, params?: { root?: string }) => {
      if (params?.root === 'C:\\demo\\a') {
        return new Promise<WorkspaceConfigReport>((_resolve, reject) => {
          rejectA = reject;
        });
      }
      return Promise.resolve(reportForRoot('C:\\demo\\b'));
    });

    const { result, rerender } = renderHook(
      ({ root }: { root: string }) => useWorkspaceConfig(root),
      {
        initialProps: { root: 'C:\\demo\\a' },
      },
    );
    rerender({ root: 'C:\\demo\\b' });
    await waitFor(() => expect(result.current.data).not.toBeNull());

    await act(async () => {
      rejectA(new Error('迟到的解析失败'));
    });
    expect(result.current.error).toBeNull();
    expect(result.current.data?.config.context).toBe('C:\\demo\\b');
  });
});

// ---------------------------------------------------------------------------
// 生成绑定调用面（AC-3 回归锁定）：命令与参数换为 workspace_config 后，命令名
// 与参数逐字不变（生成绑定底层同模块 invoke，mock 机制切换后依旧生效；无裸
// invoke 字符串）。
// ---------------------------------------------------------------------------

describe('useWorkspaceConfig：生成绑定调用面', () => {
  it('经 commands.workspaceConfig typed 入口发起：命令名与参数 { root } 逐字不变', async () => {
    invokeMock.mockResolvedValue(makeReport());

    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledWith('workspace_config', { root: ROOT });

    act(() => {
      result.current.refresh();
    });
    await waitFor(() => expect(invokeMock.mock.calls.length).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('workspace_config', { root: ROOT });
    expect(invokeMock.mock.calls.every(([name]) => name === 'workspace_config')).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 经 `ipc` 的运行期校验面（change ipc-zod-runtime-schema）：`workspaceConfig`
// 的参数面放行、`extra` 动态键面不被 strict 误拒、阈值整数两侧零误报。
// ---------------------------------------------------------------------------

/** 合法 suite（三阈值 + mutation.score 均可空）。 */
function makeSuite(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    root: 'packages/desktop',
    framework: 'vite-plus',
    cwd: '.',
    config: null,
    includes: null,
    excludes: null,
    coverage: { lines: 90, branches: 85, functions: 90 },
    mutation: { cwd: null, score: 70 },
    ...overrides,
  };
}

describe('useWorkspaceConfig：经 ipc 的参数面 / 结果面', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    resetDrift();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('workspaceConfig(root) 放行：结果干净零漂移、既有表单回填不变', async () => {
    invokeMock.mockResolvedValue(makeReport({ config: makeConfig({ tests: [] as never }) }));
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(invokeMock).toHaveBeenCalledWith('workspace_config', { root: ROOT });
    expect(result.current.data?.config.schema).toBe('spec-driven');
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['空', []],
    [
      '多键',
      [
        { key: 'futureFlag', value: { nested: [1, 2] } },
        { key: 'raw', value: 'text' },
      ],
    ],
  ])('extra 动态键（%s）与 null 可空字段 → 零漂移（不被 strict 误拒）', async (_label, extra) => {
    invokeMock.mockResolvedValue(
      makeReport({
        config: makeConfig({ extra: extra as never, context: null, rules: null }),
      }),
    );
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(result.current.data?.config.extra).toEqual(extra);
    expect(driftCount()).toBe(0);
  });

  it('reject 串 → 既有 error 态文案逐字不变、零漂移', async () => {
    invokeMock.mockRejectedValue('config.json 解析失败');
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.error).not.toBeNull());

    expect(result.current.error).toBe('config.json 解析失败');
    expect(driftCount()).toBe(0);
  });

  it('结果面脏（tests 非数组）→ 漂移恰 1 条、配置表单不崩', async () => {
    invokeMock.mockResolvedValue(makeReport({ config: makeConfig({ tests: 42 as never }) }));
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('workspaceConfig');
    expect(driftReports()[0].path).toBe('#/config/tests');
    expect(result.current.error).toBeNull();
  });

  it.each([
    ['下界 0', 0],
    ['上界 100', 100],
  ])('覆盖率阈值取%s → 零漂移（int 边界两侧）', async (_label, value) => {
    invokeMock.mockResolvedValue(
      makeReport({
        config: makeConfig({
          tests: [
            makeSuite({
              coverage: { lines: value, branches: value, functions: value },
            }) as never,
          ],
        }),
      }),
    );
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(result.current.data?.config.tests).toHaveLength(1);
    expect(driftCount()).toBe(0);
  });

  it('结果面顶层多余键 → 漂移 1 条（strictObject 生效）、渲染不变', async () => {
    invokeMock.mockResolvedValue({ ...makeReport(), surplusKey: 'x' });
    const { result } = renderHook(() => useWorkspaceConfig(ROOT));
    await waitFor(() => expect(result.current.data).not.toBeNull());

    expect(driftCount()).toBe(1);
    expect(driftReports()[0].path).toBe('#');
    expect(result.current.data?.config.schema).toBe('spec-driven');
  });
});
