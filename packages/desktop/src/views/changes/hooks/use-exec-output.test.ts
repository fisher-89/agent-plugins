import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { useExecOutput } from './use-exec-output';

// ---------------------------------------------------------------------------
// useExecOutput 单测（change-flow-show-step-only，AC-5）：exec 输出记录取数
// 单点——经 `commands.readExecOutput(root, changeName, execId)`（真实
// bindings 调用路径不 mock）；进程边界只 mock invoke（`@tauri-apps/api/core`），
// 可编程 resolve（String）/ miss（null）/ reject（字符串）并记录入参。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\flow';
const CHANGE_NAME = 'add-feature';
const EXEC_ID = 'exec-1727000000000-1';

interface Params {
  root: string | null;
  changeName: string | null;
  execId: string | null;
}

function propsWith(overrides: Partial<Params> = {}): Params {
  return { root: ROOT, changeName: CHANGE_NAME, execId: EXEC_ID, ...overrides };
}

/** 单次 read_exec_output 应答（string = 命中、null = miss、Error = reject）。 */
let outputResult: string | null | Error = null;

function readCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === 'read_exec_output');
}

beforeEach(() => {
  outputResult = 'script output\n';
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'read_exec_output') {
      return outputResult instanceof Error
        ? Promise.reject(outputResult.message)
        : Promise.resolve(outputResult);
    }
    return Promise.resolve(null);
  });
});

afterEach(() => {
  invokeMock.mockReset();
});

async function mounted(overrides: Partial<Params> = {}) {
  const rendered = renderHook((input: Params) => useExecOutput(input), {
    initialProps: propsWith(overrides),
  });
  await act(async () => {});
  return rendered;
}

describe('useExecOutput：取数（命中 / miss / 重查）', () => {
  it('正向：三参齐全 → invoke read_exec_output { root, changeName, execId }，text = 返回串、loading false、error null', async () => {
    outputResult = 'clippy: 3 warnings\n';
    const { result } = await mounted();

    await waitFor(() => expect(result.current.text).toBe('clippy: 3 warnings\n'));
    expect(readCalls()).toEqual([
      ['read_exec_output', { root: ROOT, changeName: CHANGE_NAME, execId: EXEC_ID }],
    ]);
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('正向：返回 None（miss：未知 / 非法 exec_id）→ text null、loading false、error null（空态语义）', async () => {
    outputResult = null;
    const { result } = await mounted();

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.text).toBeNull();
    expect(result.current.error).toBeNull();
    expect(readCalls()).toHaveLength(1);
  });

  it('正向：execId 变化（rerender）→ 重查新 execId 并采纳新结果', async () => {
    outputResult = 'first\n';
    const { result, rerender } = await mounted();
    await waitFor(() => expect(result.current.text).toBe('first\n'));

    outputResult = 'second\n';
    rerender(propsWith({ execId: 'exec-1727000000000-2' }));
    await waitFor(() => expect(result.current.text).toBe('second\n'));

    const calls = readCalls();
    expect(calls).toHaveLength(2);
    expect(calls[0]).toEqual([
      'read_exec_output',
      { root: ROOT, changeName: CHANGE_NAME, execId: EXEC_ID },
    ]);
    expect(calls[1]).toEqual([
      'read_exec_output',
      { root: ROOT, changeName: CHANGE_NAME, execId: 'exec-1727000000000-2' },
    ]);
    expect(result.current.error).toBeNull();
  });
});

describe('useExecOutput：异常与边界', () => {
  it('异常：invoke 拒绝 → error 记因串、text null、loading false（不炸）', async () => {
    outputResult = new Error('读输出记录失败: db 读失败');
    const { result } = await mounted();

    await waitFor(() => expect(result.current.error).toBe('读输出记录失败: db 读失败'));
    expect(result.current.text).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('异常：竞态守卫——切走 execId 后旧查询后到不回写 text / error（disposed 清理生效）', async () => {
    interface Pending {
      execId: string;
      resolve: (value: string | null) => void;
      reject: (cause: string) => void;
    }
    const pending: Pending[] = [];
    invokeMock.mockImplementation((command: string, args: Record<string, unknown>) => {
      void command;
      return new Promise((resolve, reject) => {
        pending.push({ execId: String(args.execId), resolve, reject });
      });
    });

    const { result, rerender } = await mounted();
    await waitFor(() => expect(pending).toHaveLength(1));
    expect(pending[0].execId).toBe(EXEC_ID);

    // 切走 execId：新查询发起（旧查询作废）
    rerender(propsWith({ execId: 'exec-1727000000000-2' }));
    await waitFor(() => expect(pending).toHaveLength(2));
    expect(pending[1].execId).toBe('exec-1727000000000-2');
    // 新查询先到 → 采纳
    await act(async () => {
      pending[1].resolve('new-text\n');
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.text).toBe('new-text\n'));

    // 再切一次 execId：前两查询均已作废
    rerender(propsWith({ execId: 'exec-1727000000000-3' }));
    await waitFor(() => expect(pending).toHaveLength(3));

    // 作废查询后到（resolve / reject 两臂）→ 均不回写 text / error
    await act(async () => {
      pending[0].resolve('stale-text\n');
      pending[1].reject('stale-error');
      await Promise.resolve();
    });
    expect(result.current.text).toBe('new-text\n');
    expect(result.current.error).toBeNull();

    // 在飞查询（exec-3）仍可正常落定（作废臂不干扰当前面）
    await act(async () => {
      pending[2].resolve('latest-text\n');
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.text).toBe('latest-text\n'));
    expect(result.current.error).toBeNull();
  });

  it('边界：任一入参 null（root / changeName / execId）→ 清空态零 invoke', async () => {
    const absentCases: Partial<Params>[] = [{ root: null }, { changeName: null }, { execId: null }];
    for (const absent of absentCases) {
      invokeMock.mockClear();
      const { result, unmount } = await mounted(absent);
      expect(readCalls()).toHaveLength(0);
      expect(result.current.text).toBeNull();
      expect(result.current.loading).toBe(false);
      expect(result.current.error).toBeNull();
      unmount();
    }
  });

  it('边界：空串 changeName / execId（非 null）→ 按非空参数照常发起取数', async () => {
    outputResult = null;
    const { result } = await mounted({ changeName: '', execId: '' });

    await waitFor(() => expect(readCalls()).toHaveLength(1));
    expect(readCalls()[0]).toEqual([
      'read_exec_output',
      { root: ROOT, changeName: '', execId: '' },
    ]);
    expect(result.current.text).toBeNull();
    expect(result.current.error).toBeNull();
  });
});
