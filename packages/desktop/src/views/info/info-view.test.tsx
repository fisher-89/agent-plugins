// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { CodeStatsReport } from '../../types/generated/bindings';
import { InfoView } from './info-view';

// ---------------------------------------------------------------------------
// 进程边界 Mock：IPC 收敛于 @tauri-apps/api/core invoke（useCodeStats 经生成
// 绑定 commands.codeStats 调用，底层同模块 invoke，mock 切换后依旧生效）；
// 内部模块（useCodeStats / StatsSummary / LanguageTable / DirTree）不 mock，
// 真实组合。invoke 按命令名 codeStats 分发（resolve / reject / 手动 pending
// 控制 loading 态），返回 camelCase 三面 DTO fixture。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

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
        name: 'src',
        path: 'src',
        files: 9,
        code: 300,
        comments: 35,
        blanks: 18,
        children: [
          {
            name: 'deep',
            path: 'src/deep',
            files: 2,
            code: 80,
            comments: 5,
            blanks: 3,
            children: [],
          },
        ],
      },
    ],
    ...overrides,
  };
}

/** 以 data-path 定位目录行（默认展开前 2 级：src 与 src/deep 行均在场）。 */
function rowByPath(path: string): HTMLElement | null {
  return (
    screen
      .queryAllByTestId('info-dir-node')
      .find((row) => row.getAttribute('data-path') === path) ?? null
  );
}

/** mock.calls 中某命令的调用次数。 */
function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe('InfoView：三面呈现与深度控件（AC-3/AC-4）', () => {
  it('codeStats 返回三面 DTO：汇总、语言表、目录树全部呈现，挂载恰发起一次 (root, depth=5)', async () => {
    invokeMock.mockResolvedValue(makeReport());
    render(<InfoView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('info-summary') !== null).toBe(true));
    expect(screen.getAllByTestId('info-language-row')).toHaveLength(2);
    expect(rowByPath('src') !== null).toBe(true);
    expect(rowByPath('src/deep') !== null).toBe(true);
    expect(screen.queryByTestId('info-empty')).toBeNull();
    expect(screen.queryByTestId('info-error')).toBeNull();
    expect(countOf('code_stats')).toBe(1);
    expect(invokeMock).toHaveBeenCalledWith('code_stats', { root: ROOT, depth: 5 });
  });

  it('深度 select 默认值 5；改为 8 后恰以 depth=8 重发一次，树面随新数据收缩呈现（AC-4）', async () => {
    const shallowTree = [
      { name: 'src', path: 'src', files: 9, code: 300, comments: 35, blanks: 18, children: [] },
    ];
    invokeMock.mockImplementation((_command: string, params?: { depth?: number }) =>
      Promise.resolve(
        (params?.depth ?? 5) === 5 ? makeReport() : makeReport({ tree: shallowTree }),
      ),
    );
    render(<InfoView root={ROOT} />);
    await waitFor(() => expect(rowByPath('src/deep') !== null).toBe(true));

    const select = screen.getByTestId<HTMLSelectElement>('info-depth-select');
    expect(select.value).toBe('5');
    fireEvent.change(select, { target: { value: '8' } });

    await waitFor(() => expect(countOf('code_stats')).toBe(2));
    expect(invokeMock).toHaveBeenLastCalledWith('code_stats', { root: ROOT, depth: 8 });
    await waitFor(() => expect(rowByPath('src/deep')).toBeNull());
    expect(rowByPath('src') !== null).toBe(true);
    expect(select.value).toBe('8');
  });

  it('loading 期间刷新钮 disabled（pending promise 保持）；loading 解除后可点、点击恰再发一次（AC-6）', async () => {
    let resolveLoad!: (value: CodeStatsReport) => void;
    invokeMock.mockImplementation(
      () =>
        new Promise<CodeStatsReport>((resolve) => {
          resolveLoad = resolve;
        }),
    );
    render(<InfoView root={ROOT} />);

    const refresh = screen.getByTestId<HTMLButtonElement>('info-refresh');
    expect(refresh.disabled).toBe(true);

    await act(async () => {
      resolveLoad(makeReport());
    });
    await waitFor(() => expect(refresh.disabled).toBe(false));

    const before = countOf('code_stats');
    fireEvent.click(refresh);
    await waitFor(() => expect(countOf('code_stats')).toBe(before + 1));
  });
});

describe('InfoView：错误与空态（AC-7）', () => {
  it('codeStats reject：info-error 在场且 inline 持久、无 sonner toast 节点、三面呈现区不在场', async () => {
    invokeMock.mockRejectedValue(new Error('IPC 断开'));
    render(<InfoView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('info-error') !== null).toBe(true));
    expect(screen.getByTestId('info-error').textContent).toContain('解析失败');
    expect(screen.getByTestId('info-error').textContent).toContain('IPC 断开');

    // inline 持久呈现：静置后仍在场，且无 toast 顶替（无 toast 节点出现）
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(screen.getByTestId('info-error') !== null).toBe(true);
    expect(document.querySelector('[data-sonner-toast]')).toBeNull();

    // 三面呈现区不在场、loading 复位
    expect(screen.queryByTestId('info-summary')).toBeNull();
    expect(screen.queryByTestId('info-language-table')).toBeNull();
    expect(screen.queryByTestId('info-dir-tree')).toBeNull();
    expect(screen.queryByTestId('info-loading')).toBeNull();
  });

  it('空 report（totals 全 0、languages 空数组、tree 空数组）：info-empty 在场、info-error 不在场（空态而非错误）', async () => {
    invokeMock.mockResolvedValue(
      makeReport({
        totals: { files: 0, code: 0, comments: 0, blanks: 0 },
        languages: [],
        tree: [],
      }),
    );
    render(<InfoView root={ROOT} />);

    await waitFor(() => expect(screen.getByTestId('info-empty') !== null).toBe(true));
    expect(screen.getByTestId('info-empty').textContent).toContain('未识别到代码文件');
    expect(screen.queryByTestId('info-error')).toBeNull();
    expect(screen.queryByTestId('info-summary')).toBeNull();
  });

  it('错误后点刷新且成功：info-error 消失、三面呈现恢复（inline 可恢复语义）', async () => {
    invokeMock.mockRejectedValueOnce(new Error('IPC 断开')).mockResolvedValueOnce(makeReport());
    render(<InfoView root={ROOT} />);
    await waitFor(() => expect(screen.getByTestId('info-error') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('info-refresh'));

    await waitFor(() => expect(screen.queryByTestId('info-error')).toBeNull());
    expect(screen.getByTestId('info-summary') !== null).toBe(true);
    expect(screen.getAllByTestId('info-language-row')).toHaveLength(2);
    expect(countOf('code_stats')).toBe(2);
  });
});
