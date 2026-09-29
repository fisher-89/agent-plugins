// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { DirNode } from '../../../types/generated/bindings';
import { DirTree } from './dir-tree';

// ---------------------------------------------------------------------------
// fixture：纯 props 驱动组件（展开态为组件内部 state），jsdom 真实渲染 +
// fireEvent 断言 DOM，零 mock
// ---------------------------------------------------------------------------

function node(path: string, overrides: Partial<DirNode> = {}): DirNode {
  const segments = path.split('/');
  return {
    name: segments[segments.length - 1],
    path,
    files: 1,
    code: 100,
    comments: 2,
    blanks: 3,
    children: [],
    ...overrides,
  };
}

/** 以 data-path 定位目录行（同名目录由 path 区分）。 */
function rowByPath(path: string): HTMLElement | null {
  return (
    screen
      .queryAllByTestId('info-dir-node')
      .find((row) => row.getAttribute('data-path') === path) ?? null
  );
}

function toggleOf(row: HTMLElement): HTMLElement {
  return within(row).getByTestId('info-dir-toggle');
}

describe('DirTree：展开折叠与 DOM 收敛（AC-5）', () => {
  it('默认展开前 2 级：路径段数 ≤2 的节点子行在场，第 3 级子行不在 DOM；行 info-dir-node 与 data-path 正确', () => {
    const deepest = node('src/deep/deeper/deepest');
    const deeper = node('src/deep/deeper', { children: [deepest], files: 1 });
    const deep = node('src/deep', { children: [deeper], files: 2 });
    const src = node('src', { children: [deep], files: 3, code: 300 });
    render(<DirTree nodes={[src, node('util')]} />);

    const srcRow = rowByPath('src');
    expect(srcRow !== null).toBe(true);
    expect(srcRow?.getAttribute('data-path')).toBe('src');
    expect(srcRow?.textContent).toContain('src');
    // 段数 ≤2 的节点子行在场（src 与 src/deep 均默认展开）
    expect(rowByPath('src/deep') !== null).toBe(true);
    expect(rowByPath('src/deep/deeper') !== null).toBe(true);
    expect(rowByPath('util') !== null).toBe(true);
    // 第 3 级节点（deeper）默认折叠：其子行（deepest）不在 DOM
    expect(rowByPath('src/deep/deeper/deepest')).toBeNull();
    const deeperRow = rowByPath('src/deep/deeper');
    expect(toggleOf(deeperRow!).getAttribute('aria-expanded')).toBe('false');
  });

  it('逐级点击折叠节点 → 子树行出现；再点击 → 子树行从 DOM 消失（按 data-path 查询为零）', () => {
    const deepest = node('src/deep/deeper/deepest');
    const deeper = node('src/deep/deeper', { children: [deepest], files: 1 });
    const deep = node('src/deep', { children: [deeper], files: 2 });
    const src = node('src', { children: [deep], files: 3 });
    render(<DirTree nodes={[src]} />);

    fireEvent.click(toggleOf(rowByPath('src/deep/deeper')!));
    expect(rowByPath('src/deep/deeper/deepest') !== null).toBe(true);
    expect(toggleOf(rowByPath('src/deep/deeper')!).getAttribute('aria-expanded')).toBe('true');

    fireEvent.click(toggleOf(rowByPath('src/deep/deeper')!));
    expect(rowByPath('src/deep/deeper/deepest')).toBeNull();
    expect(toggleOf(rowByPath('src/deep/deeper')!).getAttribute('aria-expanded')).toBe('false');
  });

  it('空数组：无行渲染不崩', () => {
    render(<DirTree nodes={[]} />);

    expect(screen.getByTestId('info-dir-tree') !== null).toBe(true);
    expect(screen.queryAllByTestId('info-dir-node')).toHaveLength(0);
  });

  it('叶子节点（children 空）：点击展开开关不产生子行、不崩', () => {
    render(<DirTree nodes={[node('only', { files: 2 })]} />);

    const onlyRow = rowByPath('only');
    expect(onlyRow !== null).toBe(true);
    fireEvent.click(toggleOf(onlyRow!));
    expect(screen.queryAllByTestId('info-dir-node')).toHaveLength(1);
  });

  it('大树（50 节点链）默认态：DOM 行数收敛于已展开子树（默认折叠收敛断言）', () => {
    const chainPath = (level: number): string =>
      Array.from({ length: level }, (_, k) => `a-${k + 1}`).join('/');
    let chain = node(chainPath(50), { files: 1 });
    for (let level = 49; level >= 1; level--) {
      chain = node(chainPath(level), { children: [chain], files: 50 - level + 1 });
    }
    render(<DirTree nodes={[chain]} />);

    // 默认仅前 2 级展开：可见行为第 1–3 级节点，其余 47 节点不渲染
    expect(screen.getAllByTestId('info-dir-node')).toHaveLength(3);
    expect(rowByPath('a-1/a-2/a-3') !== null).toBe(true);
    expect(rowByPath('a-1/a-2/a-3/a-4')).toBeNull();
  });

  it('深树（十段路径，对应 depth 上限 10）逐级展开后最深行可达', () => {
    const chainPath = (level: number): string =>
      Array.from({ length: level }, (_, k) => `p-${k + 1}`).join('/');
    let deep = node(chainPath(10));
    for (let level = 9; level >= 1; level--) {
      deep = node(chainPath(level), { children: [deep] });
    }
    render(<DirTree nodes={[deep]} />);
    expect(rowByPath(chainPath(10))).toBeNull();

    for (let level = 3; level <= 9; level++) {
      fireEvent.click(toggleOf(rowByPath(chainPath(level))!));
    }
    expect(rowByPath(chainPath(10)) !== null).toBe(true);
  });

  it('同名目录不同路径（a/x 与 b/x）：折叠 / 展开互不串扰（per-path 键）', () => {
    const aLeaf = node('a/x/leaf');
    const bLeaf = node('b/x/leaf');
    const a = node('a', { children: [node('a/x', { children: [aLeaf] })] });
    const b = node('b', { children: [node('b/x', { children: [bLeaf] })] });
    render(<DirTree nodes={[a, b]} />);

    // a/x 与 b/x 段数均 ≤2 默认展开：两个叶子行都在场
    expect(rowByPath('a/x/leaf') !== null).toBe(true);
    expect(rowByPath('b/x/leaf') !== null).toBe(true);

    fireEvent.click(toggleOf(rowByPath('a/x')!));
    expect(rowByPath('a/x/leaf')).toBeNull();
    expect(rowByPath('b/x/leaf') !== null).toBe(true);

    fireEvent.click(toggleOf(rowByPath('a/x')!));
    expect(rowByPath('a/x/leaf') !== null).toBe(true);
  });

  it('rerender 同 path 集合、新统计值：展开态保持（键稳定，不随重解析重置）', () => {
    const before = node('src', {
      children: [node('src/deep', { files: 2 })],
      files: 3,
      code: 300,
    });
    const { rerender } = render(<DirTree nodes={[before]} />);

    fireEvent.click(toggleOf(rowByPath('src')!));
    expect(rowByPath('src/deep')).toBeNull();

    // 模拟调深 / 刷新后的重解析：同名同 path、统计值更新
    const after = node('src', {
      children: [node('src/deep', { files: 9 })],
      files: 12,
      code: 900,
    });
    rerender(<DirTree nodes={[after]} />);

    expect(rowByPath('src/deep')).toBeNull();
    expect(rowByPath('src')?.textContent).toContain('12');

    fireEvent.click(toggleOf(rowByPath('src')!));
    expect(rowByPath('src/deep') !== null).toBe(true);
  });
});
