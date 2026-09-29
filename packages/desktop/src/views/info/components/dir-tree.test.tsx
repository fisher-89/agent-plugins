// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { DirNode, FileNode, TreeEntry } from '../../../types/generated/bindings';
import { DirTree } from './dir-tree';

// ---------------------------------------------------------------------------
// fixture：纯 props 驱动组件（展开态为组件内部 state），jsdom 真实渲染 +
// fireEvent 断言 DOM，零 mock
// ---------------------------------------------------------------------------

function dirNode(path: string, overrides: Partial<DirNode> = {}): DirNode {
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

/** 目录条目（children 为混合条目：目录先于文件） */
function dir(path: string, overrides: Partial<DirNode> = {}): TreeEntry {
  return { kind: 'dir', node: dirNode(path, overrides) };
}

/** 文件叶条目 */
function file(path: string, overrides: Partial<FileNode> = {}): TreeEntry {
  const segments = path.split('/');
  return {
    kind: 'file',
    node: {
      name: segments[segments.length - 1],
      path,
      code: 100,
      comments: 2,
      blanks: 3,
      ...overrides,
    },
  };
}

/** 以 data-path 定位行（目录行与文件叶行统一寻址；同名由 path 区分）。 */
function rowByPath(path: string): HTMLElement | null {
  const rows = [
    ...screen.queryAllByTestId('info-dir-node'),
    ...screen.queryAllByTestId('info-file-node'),
  ];
  return rows.find((row) => row.getAttribute('data-path') === path) ?? null;
}

function toggleOf(row: HTMLElement): HTMLElement {
  return within(row).getByTestId('info-dir-toggle');
}

describe('DirTree：展开折叠与 DOM 收敛（AC-5）', () => {
  it('默认展开前 2 级：路径段数 ≤2 的节点子行在场，第 3 级子行不在 DOM；行 info-dir-node 与 data-path 正确', () => {
    const deepest = dir('src/deep/deeper/deepest');
    const deeper = dir('src/deep/deeper', { children: [deepest], files: 1 });
    const deep = dir('src/deep', { children: [deeper], files: 2 });
    const src = dir('src', { children: [deep], files: 3, code: 300 });
    render(<DirTree entries={[src, dir('util')]} />);

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
    const deepest = dir('src/deep/deeper/deepest');
    const deeper = dir('src/deep/deeper', { children: [deepest], files: 1 });
    const deep = dir('src/deep', { children: [deeper], files: 2 });
    const src = dir('src', { children: [deep], files: 3 });
    render(<DirTree entries={[src]} />);

    fireEvent.click(toggleOf(rowByPath('src/deep/deeper')!));
    expect(rowByPath('src/deep/deeper/deepest') !== null).toBe(true);
    expect(toggleOf(rowByPath('src/deep/deeper')!).getAttribute('aria-expanded')).toBe('true');

    fireEvent.click(toggleOf(rowByPath('src/deep/deeper')!));
    expect(rowByPath('src/deep/deeper/deepest')).toBeNull();
    expect(toggleOf(rowByPath('src/deep/deeper')!).getAttribute('aria-expanded')).toBe('false');
  });

  it('空数组：无行渲染不崩', () => {
    render(<DirTree entries={[]} />);

    expect(screen.getByTestId('info-dir-tree') !== null).toBe(true);
    expect(screen.queryAllByTestId('info-dir-node')).toHaveLength(0);
    expect(screen.queryAllByTestId('info-file-node')).toHaveLength(0);
  });

  it('叶子节点（children 空）：点击展开开关不产生子行、不崩', () => {
    render(<DirTree entries={[dir('only', { files: 2 })]} />);

    const onlyRow = rowByPath('only');
    expect(onlyRow !== null).toBe(true);
    fireEvent.click(toggleOf(onlyRow!));
    expect(screen.queryAllByTestId('info-dir-node')).toHaveLength(1);
    expect(screen.queryAllByTestId('info-file-node')).toHaveLength(0);
  });

  it('大树（50 节点链）默认态：DOM 行数收敛于已展开子树（默认折叠收敛断言）', () => {
    const chainPath = (level: number): string =>
      Array.from({ length: level }, (_, k) => `a-${k + 1}`).join('/');
    let chain = dir(chainPath(50), { files: 1 });
    for (let level = 49; level >= 1; level--) {
      chain = dir(chainPath(level), { children: [chain], files: 50 - level + 1 });
    }
    render(<DirTree entries={[chain]} />);

    // 默认仅前 2 级展开：可见行为第 1–3 级节点，其余 47 节点不渲染
    expect(screen.getAllByTestId('info-dir-node')).toHaveLength(3);
    expect(rowByPath('a-1/a-2/a-3') !== null).toBe(true);
    expect(rowByPath('a-1/a-2/a-3/a-4')).toBeNull();
  });

  it('深树（十段路径，对应 depth 上限 10）逐级展开后最深行可达', () => {
    const chainPath = (level: number): string =>
      Array.from({ length: level }, (_, k) => `p-${k + 1}`).join('/');
    let deep = dir(chainPath(10));
    for (let level = 9; level >= 1; level--) {
      deep = dir(chainPath(level), { children: [deep] });
    }
    render(<DirTree entries={[deep]} />);
    expect(rowByPath(chainPath(10))).toBeNull();

    for (let level = 3; level <= 9; level++) {
      fireEvent.click(toggleOf(rowByPath(chainPath(level))!));
    }
    expect(rowByPath(chainPath(10)) !== null).toBe(true);
  });

  it('同名目录不同路径（a/x 与 b/x）：折叠 / 展开互不串扰（per-path 键）', () => {
    const aLeaf = dir('a/x/leaf');
    const bLeaf = dir('b/x/leaf');
    const a = dir('a', { children: [dir('a/x', { children: [aLeaf] })] });
    const b = dir('b', { children: [dir('b/x', { children: [bLeaf] })] });
    render(<DirTree entries={[a, b]} />);

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
    const before = dir('src', {
      children: [dir('src/deep', { files: 2 })],
      files: 3,
      code: 300,
    });
    const { rerender } = render(<DirTree entries={[before]} />);

    fireEvent.click(toggleOf(rowByPath('src')!));
    expect(rowByPath('src/deep')).toBeNull();

    // 模拟调深 / 刷新后的重解析：同名同 path、统计值更新
    const after = dir('src', {
      children: [dir('src/deep', { files: 9 })],
      files: 12,
      code: 900,
    });
    rerender(<DirTree entries={[after]} />);

    expect(rowByPath('src/deep')).toBeNull();
    expect(rowByPath('src')?.textContent).toContain('12');

    fireEvent.click(toggleOf(rowByPath('src')!));
    expect(rowByPath('src/deep') !== null).toBe(true);
  });
});

describe('DirTree：文件叶行（展开到文件层）', () => {
  it('目录展开后直属文件叶行在场（info-file-node + data-path + 代码行指标）；折叠后从 DOM 消失', () => {
    const src = dir('src', {
      children: [dir('src/util', { children: [] }), file('src/main.rs', { code: 250 })],
      files: 2,
    });
    render(<DirTree entries={[src]} />);

    // src 默认展开（段数 1 ≤ 2）：直属文件叶行在场
    const fileRow = rowByPath('src/main.rs');
    expect(fileRow !== null).toBe(true);
    expect(fileRow?.textContent).toContain('main.rs');
    expect(fileRow?.textContent).toContain('250');
    // 数据序即渲染序：目录先于文件
    expect(
      screen.getAllByTestId(/info-(dir|file)-node/).map((row) => row.getAttribute('data-path')),
    ).toEqual(['src', 'src/util', 'src/main.rs']);

    fireEvent.click(toggleOf(rowByPath('src')!));
    expect(rowByPath('src/main.rs')).toBeNull();
    expect(rowByPath('src/util')).toBeNull();
  });

  it('文件叶行无展开开关：行内无 info-dir-toggle，点击行不产生子行', () => {
    render(<DirTree entries={[dir('pkg', { children: [file('pkg/a.ts', { code: 5 })] })]} />);

    const fileRow = rowByPath('pkg/a.ts');
    expect(fileRow !== null).toBe(true);
    expect(within(fileRow!).queryByTestId('info-dir-toggle')).toBeNull();

    fireEvent.click(fileRow!);
    expect(screen.getAllByTestId('info-file-node')).toHaveLength(1);
  });

  it('根层直属文件叶为顶层行恒在场（不随目录折叠消失），与目录行互不干扰', () => {
    const src = dir('src', { children: [file('src/a.rs', { code: 80 })], files: 1 });
    const readme = file('README.md', { code: 40 });
    render(<DirTree entries={[src, readme]} />);

    expect(rowByPath('README.md') !== null).toBe(true);
    expect(rowByPath('README.md')?.textContent).toContain('40');

    // 折叠 src 只影响其子树：根层文件叶不受 per-path 折叠态波及
    fireEvent.click(toggleOf(rowByPath('src')!));
    expect(rowByPath('src/a.rs')).toBeNull();
    expect(rowByPath('README.md') !== null).toBe(true);
  });

  it('仅文件叶无目录（扁平工作区）：无目录行、文件行全量呈现不崩', () => {
    render(<DirTree entries={[file('a.rs', { code: 10 }), file('b.ts', { code: 20 })]} />);

    expect(screen.queryAllByTestId('info-dir-node')).toHaveLength(0);
    expect(screen.getAllByTestId('info-file-node')).toHaveLength(2);
    expect(rowByPath('b.ts')?.textContent).toContain('20');
  });
});
