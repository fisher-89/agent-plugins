import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { StdoutPanel } from './stdout-panel';

// ---------------------------------------------------------------------------
// StdoutPanel 单测（change-flow-show-step-only，AC-5）：单列 `<pre>` 渲染脚本
// 真实输出（static-check 的 2>&1 交错流 / test-execution 的合并流），miss 呈
// 空态、invoke 拒绝呈错误态——不炸图。进程边界 mock：vi.mock 控
// `read_exec_output` 返回 Some(文本) / None / reject；内部模块不 mock
//（真实组合 useExecOutput）。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\flow';
const CHANGE_NAME = 'add-feature';
const EXEC_ID = 'exec-1727000000000-3';

/** read_exec_output 应答：string = 命中（UTF-8 lossy 文本）、null = miss、Error = reject。 */
let outputResult: string | null | Error = null;

beforeEach(() => {
  outputResult = null;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string, args: Record<string, unknown>) => {
    if (command === 'read_exec_output') {
      void args;
      return outputResult instanceof Error
        ? Promise.reject(outputResult.message)
        : Promise.resolve(outputResult);
    }
    return Promise.resolve(null);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

function renderPanel(root: string | null = ROOT) {
  return render(<StdoutPanel root={root} changeName={CHANGE_NAME} execId={EXEC_ID} />);
}

describe('StdoutPanel：输出渲染（命中 / 空态 / 错误态）', () => {
  it('正向：read_exec_output 命中 → 单列 <pre> 渲染 UTF-8 lossy 文本（含 emoji 与替换符）', async () => {
    outputResult = 'line-1\n行二 🚀\n\uFFFD 替换符\n';
    renderPanel();

    const pre = await screen.findByTestId('stdout-text');
    expect(pre.tagName).toBe('PRE');
    expect(pre.textContent).toBe('line-1\n行二 🚀\n\uFFFD 替换符\n');
    expect(screen.queryByTestId('stdout-empty')).toBeNull();
    expect(screen.queryByTestId('stdout-error')).toBeNull();
    // 取数定式：磁盘面按 change name（非 id）+ execId 寻址
    expect(invokeMock).toHaveBeenCalledWith('read_exec_output', {
      root: ROOT,
      changeName: CHANGE_NAME,
      execId: EXEC_ID,
    });
  });

  it('正向：text === null（miss，未知 / 非法 exec_id）→ 空态提示，不炸图', async () => {
    outputResult = null;
    renderPanel();

    const empty = await screen.findByTestId('stdout-empty');
    expect(empty.textContent).toBe('（无输出记录）');
    expect(screen.queryByTestId('stdout-text')).toBeNull();
    expect(screen.queryByTestId('stdout-error')).toBeNull();
    expect(screen.getByTestId('stdout-panel') !== null).toBe(true);
  });

  it('异常：invoke 拒绝 → 错误态提示（记因串呈现），不炸图', async () => {
    outputResult = new Error('读输出记录失败: 磁盘不可读');
    renderPanel();

    const error = await screen.findByTestId('stdout-error');
    expect(error.textContent).toBe('读输出记录失败: 磁盘不可读');
    expect(screen.queryByTestId('stdout-text')).toBeNull();
    expect(screen.queryByTestId('stdout-empty')).toBeNull();
    // 面板骨架仍在（错误不阻断抽屉其余内容）
    expect(screen.getByTestId('stdout-panel') !== null).toBe(true);
  });

  it('异常：root === null → 空态（零 invoke，不崩溃）', async () => {
    renderPanel(null);

    await waitFor(() => expect(screen.getByTestId('stdout-empty') !== null).toBe(true));
    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByTestId('stdout-panel').textContent).toContain('执行输出');
  });

  it('边界：超长输出（>1000 chars）→ <pre> 完整渲染不截断；空串输出 → 渲染合法空 <pre>', async () => {
    const long = `${'x'.repeat(1200)}\n尾行标记`;
    outputResult = long;
    const first = renderPanel();
    const pre = await screen.findByTestId('stdout-text');
    expect(pre.textContent).toBe(long);
    expect(pre.textContent?.length).toBe(long.length);
    expect(pre.textContent).toContain('尾行标记');
    first.unmount();

    // 空串（非 null）→ 命中语义：渲染空 <pre>（不落空态占位）
    outputResult = '';
    renderPanel();
    const blank = await screen.findByTestId('stdout-text');
    expect(blank.textContent).toBe('');
    expect(screen.queryByTestId('stdout-empty')).toBeNull();
  });
});
