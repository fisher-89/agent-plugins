import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiFileTree, ArchiFileEntry } from '../../../types/generated/bindings';
import { FileTree } from './file-tree';

/**
 * 左栏文件树渲染面单测（AC-4 / AC-11 / AC-12 / AC-13）：扁平条目 → 森林的
 * 层级派生与兄弟序、缺省展开三段、归属标记呈现（member 仅弱化文字）、归属行
 * 点击语义、聚焦反查（多行高亮 / 根节点展开不下钻 / 滚到首条 / 零命中静默）、空树与
 * 取数状态面（挂载取数 / 解析中 / 失败行内 / 空根零 invoke）。
 *
 * 取数面已内收到组件：`FileTree` 只收 `root`，条目经 `useArchiFileTree` 取回。
 * 进程边界 mock：`@tauri-apps/api/core` 的 `invoke`（按命令名分发文件树 fixture）。
 * 展开态是组件内部 state，断言真实 DOM（`data-testid` / `data-path` / `data-mark`）。
 * `scrollIntoView` 由本文件内的临时桩补（`afterEach` 复原，不写进 `test-setup.ts` 原型）。
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

const ROOT = 'C:\\demo\\alpha';

/** 条目构造器（七字段全给缺省值，按需覆盖）。 */
function entry(path: string, overrides: Partial<ArchiFileEntry> = {}): ArchiFileEntry {
  const name = path.split('/').at(-1) ?? path;
  return {
    path,
    name,
    kind: 'dir',
    mark: 'none',
    owners: [],
    contains: 0,
    exists: true,
    ...overrides,
  };
}

/** 目录条目（缺省形态）。 */
function dir(path: string, overrides: Partial<ArchiFileEntry> = {}): ArchiFileEntry {
  return entry(path, { kind: 'dir', ...overrides });
}

/** 文件条目（缺省形态）。 */
function file(path: string, overrides: Partial<ArchiFileEntry> = {}): ArchiFileEntry {
  return entry(path, { kind: 'file', ...overrides });
}

/** 文件树取数装置：`archi_file_tree` 回传入条目（其余命令一律拒绝）。 */
function serve(entries: ArchiFileEntry[]): void {
  invokeMock.mockImplementation((command: string) => {
    if (command === 'archi_file_tree') return Promise.resolve<ArchiFileTree>({ entries });
    return Promise.reject(new Error(`未预期命令: ${command}`));
  });
}

/** 渲染树并等待取数落定（`focused` 可空，`onFocus` 为观测桩）。 */
async function renderTree(
  entries: ArchiFileEntry[],
  focused: string | null = null,
  root: string | null = ROOT,
): Promise<{ onFocus: ReturnType<typeof vi.fn> }> {
  const onFocus = vi.fn();
  serve(entries);
  render(<FileTree root={root} focused={focused} onFocus={onFocus} />);
  // 根有效时树本体在取数回包后挂载；空根恒驻加载面，勿等树本体
  if (root !== null) await screen.findByTestId('archi-file-tree');
  return { onFocus };
}

/** 已渲染行（`archi-file-node`）。 */
function rows(): HTMLElement[] {
  return screen.queryAllByTestId('archi-file-node');
}

/** 已渲染行的 `data-path` 序列（渲染序）。 */
function renderedPaths(): string[] {
  return rows().map((row) => row.getAttribute('data-path') ?? '');
}

/** 单行元素（不在场返回 `undefined`）。 */
function rowOf(path: string): HTMLElement | undefined {
  return rows().find((row) => row.getAttribute('data-path') === path);
}

/** 行内标记元素（`none` 行零标记）。 */
function markOf(path: string): HTMLElement | null {
  const row = rowOf(path);
  if (row === undefined) return null;
  return row.querySelector<HTMLElement>('[data-testid="archi-file-mark"]');
}

/** 该行是否高亮（聚焦命中的行置独立 `bg-muted` 类；`hover:bg-muted` 不算）。 */
function isHighlighted(path: string): boolean {
  const row = rowOf(path);
  return row !== undefined && row.className.split(/\s+/).includes('bg-muted');
}

/** 文件内临时 `scrollIntoView` 桩（调用 `afterEach` 复原，不进全局 setup）。 */
function stubScroll(): ReturnType<typeof vi.fn> {
  const spy = vi.fn();
  Object.defineProperty(Element.prototype, 'scrollIntoView', {
    configurable: true,
    writable: true,
    value: spy,
  });
  return spy;
}

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  cleanup();
  // 复原：删掉本文件装上的实例级方法（jsdom 原生无该 API）
  Reflect.deleteProperty(Element.prototype, 'scrollIntoView');
});

describe('FileTree 层级派生与兄弟序', () => {
  it('应按同层目录先于文件、各自 name 序渲染（不受入参序影响）', async () => {
    await renderTree([file('src/z.ts'), dir('src/m'), file('src/a.ts'), dir('src/a'), dir('src')]);

    // 顶层只有 `src`（目录链缺席即挂根，故语料显式给出祖先目录）
    expect(renderedPaths()).toEqual(['src', 'src/a', 'src/m', 'src/a.ts', 'src/z.ts']);
  });

  it('应缺省展开路径段数 ≤ 3 的目录、更深层折叠且不渲子树 DOM', async () => {
    await renderTree([dir('a'), dir('a/b'), dir('a/b/c'), dir('a/b/c/d'), file('a/b/c/d/e.ts')]);

    // `a`（1 段）/ `a/b`（2 段）/ `a/b/c`（3 段）默认展开 ⇒ `a/b/c/d` 行可见但自身折叠
    expect(renderedPaths()).toEqual(['a', 'a/b', 'a/b/c', 'a/b/c/d']);
    expect(rowOf('a/b/c/d/e.ts')).toBeUndefined();

    fireEvent.click(screen.getByLabelText('展开 a/b/c/d'));
    expect(rowOf('a/b/c/d/e.ts')).toBeDefined();

    // 折叠后子树零节点（不是 display:none）
    fireEvent.click(screen.getByLabelText('收起 a/b/c/d'));
    expect(rowOf('a/b/c/d/e.ts')).toBeUndefined();
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(4);
  });

  it('应在展开与收起时不触发取数（纯本地 state、零 onFocus）', async () => {
    const { onFocus } = await renderTree([
      dir('a'),
      dir('a/b'),
      dir('a/b/c'),
      dir('a/b/c/d'),
      file('a/b/c/d/e.ts'),
    ]);
    const treeCalls = invokeMock.mock.calls.length;

    fireEvent.click(screen.getByLabelText('展开 a/b/c/d'));
    fireEvent.click(screen.getByLabelText('收起 a/b/c/d'));
    fireEvent.click(screen.getByLabelText('展开 a/b/c/d'));

    expect(onFocus).not.toHaveBeenCalled();
    expect(invokeMock.mock.calls).toHaveLength(treeCalls);
    // per-path 翻转集合稳定：多次翻转后仍回到展开态
    expect(rowOf('a/b/c/d/e.ts')).toBeDefined();
    // 聚焦面零命中不产生提示节点
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
  });
});

describe('FileTree 归属标记呈现', () => {
  it('应逐值渲出标记节点与 tooltip（member / none 行零标记节点）', async () => {
    await renderTree([
      dir('roots', { mark: 'elementRoot', owners: ['App.A'] }),
      dir('shared', { mark: 'ambiguous', owners: ['App.A', 'App.B'] }),
      dir('members', { mark: 'member', owners: ['App.A'] }),
      dir('container', { mark: 'containsElements', contains: 2 }),
      file('loose.ts', { mark: 'none' }),
    ]);

    const cases: [string, string, string][] = [
      ['shared', 'ambiguous', '多归属：App.A、App.B'],
      ['container', 'containsElements', '含 2 个模块'],
    ];
    for (const [path, mark, title] of cases) {
      const node = markOf(path);
      expect(node, `${path} 应有标记节点`).not.toBeNull();
      expect(node?.getAttribute('data-mark')).toBe(mark);
      expect(node?.getAttribute('title')).toBe(title);
    }
    // elementRoot 为按钮化标记：短名文案 + data-mark
    const rootMark = markOf('roots');
    expect(rootMark?.getAttribute('data-mark')).toBe('elementRoot');
    expect(rootMark?.textContent).toBe('模块 A');
    // member 仅以弱化文字表达归属（不渲标记节点）；none 行同样零标记
    for (const path of ['members', 'loose.ts']) {
      expect(markOf(path), `${path} 不应有标记节点`).toBeNull();
    }
    expect(within(rowOf('members')!).getByText('members').className).toContain(
      'text-muted-foreground',
    );
    // containsElements 的计数由标记徽章自身承载
    expect(markOf('container')?.textContent).toBe('2');
  });

  it('应对缺失目录（exists = false）渲缺失提示且不影响标记', async () => {
    await renderTree([dir('ghost', { mark: 'elementRoot', owners: ['App.Ghost'], exists: false })]);

    const row = rowOf('ghost')!;
    expect(within(row).getByTitle('声明了但磁盘上不存在').textContent).toBe('缺失');
    expect(markOf('ghost')?.getAttribute('data-mark')).toBe('elementRoot');
  });
});

describe('FileTree 行点击语义', () => {
  it('应在 elementRoot / member 行点击时恰一次 onFocus(owners[0])', async () => {
    const { onFocus } = await renderTree([
      dir('roots', { mark: 'elementRoot', owners: ['App.Core.Archi'] }),
      dir('members', { mark: 'member', owners: ['App.Core.Archi'] }),
    ]);

    fireEvent.click(rowOf('roots')!);
    expect(onFocus).toHaveBeenCalledTimes(1);
    expect(onFocus).toHaveBeenLastCalledWith('App.Core.Archi');

    fireEvent.click(rowOf('members')!);
    expect(onFocus).toHaveBeenCalledTimes(2);
    expect(onFocus).toHaveBeenLastCalledWith('App.Core.Archi');
  });

  it('应在非归属三值行点击时零 onFocus、只切展开（且不弹候选层）', async () => {
    const { onFocus } = await renderTree([
      dir('shared', { mark: 'ambiguous', owners: ['A', 'B'] }),
      dir('shared/nested'),
      dir('container', { mark: 'containsElements', contains: 1 }),
      dir('container/nested'),
      dir('plain', { mark: 'none' }),
      dir('plain/nested'),
    ]);

    // 三层语料使父目录缺省展开、子目录缺省折叠（点击父行即收起、点击子行即展开）
    for (const path of ['shared', 'container', 'plain']) {
      const child = `${path}/nested`;
      expect(rowOf(child), `${child} 缺省可见`).toBeDefined();
      fireEvent.click(rowOf(path)!);
      expect(onFocus).not.toHaveBeenCalled();
      expect(rowOf(child), `点击 ${path} 后子目录应消失（收起）`).toBeUndefined();
      fireEvent.click(rowOf(path)!);
      expect(rowOf(child), `再点 ${path} 后子目录应回来（展开）`).toBeDefined();
    }
    // 零候选弹层
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.querySelector('[popover]')).toBeNull();
  });
});

describe('FileTree 聚焦反查', () => {
  it('应在多 path 元素聚焦时同时高亮全部命中行，且不触发 onFocus', async () => {
    const { onFocus } = await renderTree(
      [
        file('src/shell/index.ts', { mark: 'elementRoot', owners: ['App.Frontend.Shell'] }),
        file('src/shell/app.ts', { mark: 'elementRoot', owners: ['App.Frontend.Shell'] }),
        file('src/shell/routes.ts', { mark: 'elementRoot', owners: ['App.Frontend.Shell'] }),
        file('src/shell/store.ts', { mark: 'elementRoot', owners: ['App.Frontend.Shell'] }),
        file('src/other.ts'),
      ],
      'App.Frontend.Shell',
    );

    for (const path of [
      'src/shell/index.ts',
      'src/shell/app.ts',
      'src/shell/routes.ts',
      'src/shell/store.ts',
    ]) {
      expect(isHighlighted(path), `${path} 应高亮`).toBe(true);
      expect(markOf(path)).not.toBeNull();
    }
    expect(isHighlighted('src/other.ts')).toBe(false);
    expect(onFocus).not.toHaveBeenCalled();
  });

  it('应仅展开聚焦模块的根节点及其祖先链、不下钻子节点（零 setState）', async () => {
    await renderTree(
      [
        dir('a', { mark: 'containsElements', contains: 1 }),
        dir('a/b/c'),
        dir('a/b/c/d', { mark: 'elementRoot', owners: ['App.Deep'] }),
        dir('a/b/c/d/inner'),
        file('a/b/c/d/e.ts', { mark: 'member', owners: ['App.Deep'] }),
        file('a/b/c/d/inner/f.ts', { mark: 'member', owners: ['App.Deep'] }),
      ],
      'App.Deep',
    );

    // 缺省只展开前 3 段 ⇒ 根节点行本不可见；forcedOpen 展开祖先链与根节点自身
    expect(rowOf('a/b/c/d')).toBeDefined();
    expect(isHighlighted('a/b/c/d')).toBe(true);
    expect(rowOf('a/b/c/d/e.ts'), '根节点的直接子行可见').toBeDefined();
    // 子目录只渲行不展开：孙节点不在 DOM（member 命中不驱动下钻）
    expect(rowOf('a/b/c/d/inner')).toBeDefined();
    expect(rowOf('a/b/c/d/inner/f.ts')).toBeUndefined();

    // 用户手动展开后，内部 member 行照常高亮
    fireEvent.click(screen.getByLabelText('展开 a/b/c/d/inner'));
    expect(isHighlighted('a/b/c/d/inner/f.ts')).toBe(true);
  });

  it('应对首条命中行调用一次 scrollIntoView({ block: nearest })', async () => {
    const scroll = stubScroll();
    await renderTree(
      [
        file('a/hit.ts', { mark: 'elementRoot', owners: ['App.Hit'] }),
        file('b/hit.ts', { mark: 'elementRoot', owners: ['App.Hit'] }),
        file('c/miss.ts'),
      ],
      'App.Hit',
    );

    expect(scroll).toHaveBeenCalledTimes(1);
    expect(scroll).toHaveBeenCalledWith({ block: 'nearest' });
    const [first] = scroll.mock.instances as HTMLElement[];
    expect(first?.getAttribute('data-path')).toBe('a/hit.ts');
  });

  it('应在换焦点时切换高亮，并在 null 时全清（零命中无提示）', async () => {
    const scroll = stubScroll();
    const entries = [
      file('a/hit.ts', { mark: 'elementRoot', owners: ['App.Hit'] }),
      file('b/miss.ts'),
    ];
    const onFocus = vi.fn();
    serve(entries);
    const { rerender } = render(<FileTree root={ROOT} focused={'App.Gone'} onFocus={onFocus} />);
    await screen.findByTestId('archi-file-tree');
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
    expect(isHighlighted('a/hit.ts')).toBe(false);

    rerender(<FileTree root={ROOT} focused={'App.Hit'} onFocus={onFocus} />);
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
    expect(isHighlighted('a/hit.ts')).toBe(true);

    rerender(<FileTree root={ROOT} focused={null} onFocus={onFocus} />);
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
    expect(isHighlighted('a/hit.ts')).toBe(false);
    expect(onFocus).not.toHaveBeenCalled();
    // 高亮是派生面，不产生额外滚动（仅首次命中一次）
    expect(scroll).toHaveBeenCalledTimes(1);
  });
});

describe('FileTree 展开态保持与空态', () => {
  it('应在同 root 重渲后保持 per-path 展开态，折叠子树仍零 DOM 节点', async () => {
    const entries = [dir('a'), dir('a/b'), dir('a/b/c'), dir('a/b/c/d'), file('a/b/c/d/e.ts')];
    const onFocus = vi.fn();
    serve(entries);
    const { rerender } = render(<FileTree root={ROOT} focused={null} onFocus={onFocus} />);
    await screen.findByTestId('archi-file-tree');
    const treeCalls = invokeMock.mock.calls.length;

    // `a/b/c/d`（4 段）缺省折叠 ⇒ 点它展开
    fireEvent.click(screen.getByLabelText('展开 a/b/c/d'));
    expect(rowOf('a/b/c/d/e.ts')).toBeDefined();

    rerender(<FileTree root={ROOT} focused={null} onFocus={onFocus} />);
    expect(rowOf('a/b/c/d/e.ts'), '重渲后 per-path 展开态保持').toBeDefined();
    expect(invokeMock.mock.calls, '同 root 重渲不重取').toHaveLength(treeCalls);

    // 收起一级目录 ⇒ 其子树整体离开 DOM
    fireEvent.click(screen.getByLabelText('收起 a'));
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(1);
    expect(rowOf('a/b')).toBeUndefined();
    expect(rowOf('a/b/c/d/e.ts')).toBeUndefined();
  });

  it('应在空条目时渲空树提示并零行节点', async () => {
    await renderTree([]);

    expect(screen.getByTestId('archi-file-tree-empty').textContent).toBe('仓库内无源文件');
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(0);
  });
});

describe('FileTree 取数状态面', () => {
  it('应在挂载时以入参 root 发起一次 archi_file_tree', async () => {
    await renderTree([file('src/a.ts')]);

    expect(invokeMock.mock.calls).toEqual([['archi_file_tree', { root: ROOT }]]);
  });

  it('应在首轮未回时渲「解析中…」（树本体不挂载）', async () => {
    invokeMock.mockImplementation(() => new Promise<ArchiFileTree>(() => {}));
    render(<FileTree root={ROOT} focused={null} onFocus={vi.fn()} />);

    await waitFor(() =>
      expect(screen.getByTestId('archi-file-tree-loading').textContent).toBe('解析中…'),
    );
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
  });

  it('应在取数失败时只渲行内错误（不落树本体）', async () => {
    invokeMock.mockImplementation(() => Promise.reject(new Error('模型结构错（共 1 条）')));
    render(<FileTree root={ROOT} focused={null} onFocus={vi.fn()} />);

    const notice = await screen.findByTestId('archi-file-tree-error');
    expect(notice.textContent).toContain('文件树取数失败');
    expect(notice.textContent).toContain('模型结构错');
    expect(notice.textContent).toContain('画布照常渲染');
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
  });

  it('应在 root 为 null 时零 invoke 并停在「解析中…」占位', async () => {
    const onFocus = vi.fn();
    render(<FileTree root={null} focused={null} onFocus={onFocus} />);

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByTestId('archi-file-tree-loading').textContent).toBe('解析中…');
    expect(screen.queryByTestId('archi-file-tree')).toBeNull();
  });

  it('应在换 root 时以新根重取并换成新条目', async () => {
    invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
      if (command !== 'archi_file_tree') {
        return Promise.reject(new Error(`未预期命令: ${command}`));
      }
      const root = typeof params?.root === 'string' ? params.root : '';
      return Promise.resolve<ArchiFileTree>({
        entries: [file(`${root}/a.ts`)],
      });
    });
    const onFocus = vi.fn();
    const { rerender } = render(<FileTree root={'A'} focused={null} onFocus={onFocus} />);
    await screen.findByTestId('archi-file-tree');
    expect(rowOf('A/a.ts')).toBeDefined();

    rerender(<FileTree root={'B'} focused={null} onFocus={onFocus} />);
    await waitFor(() => expect(rowOf('B/a.ts')).toBeDefined());
    expect(rowOf('A/a.ts')).toBeUndefined();
    expect(invokeMock.mock.calls.at(-1)).toEqual(['archi_file_tree', { root: 'B' }]);
  });
});

describe('FileTree 条目形态与规模边界', () => {
  it('应在上千条扁平条目下渲染不抛错且 data-path 唯一', async () => {
    const entries: ArchiFileEntry[] = [];
    for (let module = 0; module < 20; module += 1) {
      const root = `m${String(module).padStart(2, '0')}`;
      entries.push(
        dir(root),
        dir(`${root}/deep`),
        dir(`${root}/deep/inner`),
        dir(`${root}/deep/inner/leaf`),
      );
      for (let index = 0; index < 50; index += 1) {
        entries.push(
          file(`${root}/deep/inner/leaf/f${String(index).padStart(4, '0')}.ts`, {
            mark: 'member',
            owners: [`App.M${String(module).padStart(2, '0')}`],
          }),
        );
      }
    }
    await renderTree(entries);

    const paths = renderedPaths();
    expect(paths.length).toBeGreaterThan(0);
    expect(new Set(paths).size).toBe(paths.length);
    // 默认只展开前 3 段 ⇒ 五段文件行缺省不在 DOM（可见行数有界）
    expect(paths.length).toBeLessThan(1000);
    expect(
      paths.every((path) => path.split('/').length <= 4),
      '可见行深度不超过 4 段',
    ).toBe(true);
    // 展开一个深层目录后其文件行进 DOM（标记与展开逻辑照常；member 行零标记）
    fireEvent.click(screen.getByLabelText('展开 m00/deep/inner/leaf'));
    expect(rowOf('m00/deep/inner/leaf/f0000.ts')).toBeDefined();
    expect(markOf('m00/deep/inner/leaf/f0000.ts')).toBeNull();
  });

  it('应对空 path / 超长 / emoji / 单段 path 形态分别给出可判行为', async () => {
    const long = `src/${'长'.repeat(1100)}.ts`;
    await renderTree([file(''), file(long), file('src/空 格🚀.ts'), file('single.ts')]);

    // 空 path：不抛错、行渲出且 data-path 逐字保留
    expect(rowOf('')).toBeDefined();
    expect(rowOf('')?.getAttribute('data-path')).toBe('');
    // 超长 / emoji 与空格：data-path 逐字保留
    expect(rowOf(long)?.getAttribute('data-path')).toBe(long);
    expect(rowOf('src/空 格🚀.ts')).toBeDefined();
    // 单段 path：无 chevron（无子目录）
    const single = rowOf('single.ts')!;
    expect(within(single).queryByLabelText(/展开|收起/)).toBeNull();
  });
});
