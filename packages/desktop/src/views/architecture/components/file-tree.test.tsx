import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiFileEntry } from '../../../types/generated/bindings';
import { FileTree, TreeModeSwitch } from './file-tree';

/**
 * 左栏文件树渲染面单测（AC-4 / AC-11 / AC-12 / AC-13）：扁平条目 → 森林的
 * 层级派生与兄弟序、缺省展开两段、五种标记呈现、归属行点击语义、聚焦反查
 * （多行高亮 / 强制展开 / 滚到首条 / 零命中提示）、空树与图例、分段控件。
 *
 * 零 mock：纯 props 驱动（展开态是组件内部 state），断言真实 DOM（`data-testid` /
 * `data-path` / `data-mark` / `data-mode` 钩子）。`scrollIntoView` 由本文件内的
 * 临时桩补（`afterEach` 复原，不写进 `test-setup.ts` 原型）。
 */

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

/** 渲染树（`focused` 可空，`onFocus` 为观测桩）。 */
function renderTree(
  entries: ArchiFileEntry[],
  focused: string | null = null,
): { onFocus: ReturnType<typeof vi.fn> } {
  const onFocus = vi.fn();
  render(<FileTree entries={entries} focused={focused} onFocus={onFocus} />);
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

afterEach(() => {
  cleanup();
  // 复原：删掉本文件装上的实例级方法（jsdom 原生无该 API）
  Reflect.deleteProperty(Element.prototype, 'scrollIntoView');
});

describe('FileTree 层级派生与兄弟序', () => {
  it('应按同层目录先于文件、各自 name 序渲染（不受入参序影响）', () => {
    renderTree([file('src/z.ts'), dir('src/m'), file('src/a.ts'), dir('src/a'), dir('src')]);

    // 顶层只有 `src`（目录链缺席即挂根，故语料显式给出祖先目录）
    expect(renderedPaths()).toEqual(['src', 'src/a', 'src/m', 'src/a.ts', 'src/z.ts']);
  });

  it('应缺省展开路径段数 ≤ 2 的目录、更深层折叠且不渲子树 DOM', () => {
    renderTree([dir('a'), dir('a/b'), dir('a/b/c'), file('a/b/c/d.ts')]);

    // `a`（1 段）/ `a/b`（2 段）默认展开 ⇒ `a/b/c` 行可见但自身折叠
    expect(renderedPaths()).toEqual(['a', 'a/b', 'a/b/c']);
    expect(rowOf('a/b/c/d.ts')).toBeUndefined();

    fireEvent.click(screen.getByLabelText('展开 a/b/c'));
    expect(rowOf('a/b/c/d.ts')).toBeDefined();

    // 折叠后子树零节点（不是 display:none）
    fireEvent.click(screen.getByLabelText('收起 a/b/c'));
    expect(rowOf('a/b/c/d.ts')).toBeUndefined();
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(3);
  });

  it('应在展开与收起时不触发取数（纯本地 state、零 onFocus）', () => {
    const { onFocus } = renderTree([dir('a'), dir('a/b'), dir('a/b/c'), file('a/b/c/d.ts')]);

    fireEvent.click(screen.getByLabelText('展开 a/b/c'));
    fireEvent.click(screen.getByLabelText('收起 a/b/c'));
    fireEvent.click(screen.getByLabelText('展开 a/b/c'));

    expect(onFocus).not.toHaveBeenCalled();
    // per-path 翻转集合稳定：多次翻转后仍回到展开态
    expect(rowOf('a/b/c/d.ts')).toBeDefined();
    // 聚焦面零命中提示不因展开出现
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
  });
});

describe('FileTree 五种标记呈现', () => {
  it('应逐值渲出 data-mark 与 tooltip（none 行零标记节点）', () => {
    renderTree([
      dir('roots', { mark: 'elementRoot', owners: ['App.A'] }),
      dir('shared', { mark: 'ambiguous', owners: ['App.A', 'App.B'] }),
      dir('members', { mark: 'member', owners: ['App.A'] }),
      dir('container', { mark: 'containsElements', contains: 2 }),
      file('loose.ts', { mark: 'none' }),
    ]);

    const cases: [string, string | null, string | null][] = [
      ['roots', 'elementRoot', '模块 App.A'],
      ['shared', 'ambiguous', '多归属：App.A、App.B'],
      ['members', 'member', '属于 App.A'],
      ['container', 'containsElements', '含 2 个模块'],
      ['loose.ts', null, null],
    ];
    for (const [path, mark, title] of cases) {
      const node = markOf(path);
      if (mark === null) {
        expect(node, `${path} 为 none 行 ⇒ 零标记节点`).toBeNull();
        continue;
      }
      expect(node, `${path} 应有标记节点`).not.toBeNull();
      expect(node?.getAttribute('data-mark')).toBe(mark);
      expect(node?.getAttribute('title')).toBe(title);
    }
    // containsElements 额外渲尾随计数小字
    expect(within(rowOf('container')!).getByText('·2')).toBeTruthy();
  });

  it('应对缺失目录（exists = false）渲缺失提示且不影响标记', () => {
    renderTree([dir('ghost', { mark: 'elementRoot', owners: ['App.Ghost'], exists: false })]);

    const row = rowOf('ghost')!;
    expect(within(row).getByTitle('声明了但磁盘上不存在').textContent).toBe('缺失');
    expect(markOf('ghost')?.getAttribute('data-mark')).toBe('elementRoot');
  });
});

describe('FileTree 行点击语义', () => {
  it('应在 elementRoot / member 行点击时恰一次 onFocus(owners[0])', () => {
    const { onFocus } = renderTree([
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

  it('应在非归属三值行点击时零 onFocus、只切展开（且不弹候选层）', () => {
    const { onFocus } = renderTree([
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
  it('应在多 path 元素聚焦时同时高亮全部命中行，且不触发 onFocus', () => {
    const { onFocus } = renderTree(
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

  it('应在焦点命中深层条目时强制展开祖先链（零 setState）', () => {
    renderTree(
      [
        dir('a', { mark: 'containsElements', contains: 1 }),
        dir('a/b', { mark: 'containsElements', contains: 1 }),
        dir('a/b/c'),
        file('a/b/c/d.ts', { mark: 'member', owners: ['App.Deep'] }),
      ],
      'App.Deep',
    );

    // 缺省只展开前 2 段 ⇒ 该行本不可见；forcedOpen 派生集合把祖先链全部展开
    expect(rowOf('a/b/c/d.ts')).toBeDefined();
    expect(isHighlighted('a/b/c/d.ts')).toBe(true);
  });

  it('应对首条命中行调用一次 scrollIntoView({ block: nearest })', () => {
    const scroll = stubScroll();
    renderTree(
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

  it('应在零命中时渲顶部弱提示且不滚动（文案并列两种成因）', () => {
    const scroll = stubScroll();
    renderTree([dir('src'), file('src/a.ts')], 'App.Container');

    const tip = screen.getByTestId('archi-file-tree-focus-miss');
    expect(tip.textContent).toBe(
      '该元素在树上无可定位节点（容器元素不持有文件，或其源文件均在判定面之外）',
    );
    expect(scroll).not.toHaveBeenCalled();
    // 并列两成因：文案不得只断言「容器元素」
    expect(tip.textContent).not.toBe('该元素是容器元素');
  });

  it('应在换焦点时切换提示与高亮，并在 null 时全清', () => {
    const scroll = stubScroll();
    const entries = [
      file('a/hit.ts', { mark: 'elementRoot', owners: ['App.Hit'] }),
      file('b/miss.ts'),
    ];
    const onFocus = vi.fn();
    const { rerender } = render(
      <FileTree entries={entries} focused={'App.Gone'} onFocus={onFocus} />,
    );
    expect(screen.getByTestId('archi-file-tree-focus-miss')).toBeTruthy();
    expect(isHighlighted('a/hit.ts')).toBe(false);

    rerender(<FileTree entries={entries} focused={'App.Hit'} onFocus={onFocus} />);
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
    expect(isHighlighted('a/hit.ts')).toBe(true);

    rerender(<FileTree entries={entries} focused={null} onFocus={onFocus} />);
    expect(screen.queryByTestId('archi-file-tree-focus-miss')).toBeNull();
    expect(isHighlighted('a/hit.ts')).toBe(false);
    expect(onFocus).not.toHaveBeenCalled();
    // 提示与高亮是派生面，不产生额外滚动（仅首次命中一次）
    expect(scroll).toHaveBeenCalledTimes(1);
  });
});

describe('FileTree 展开态保持与空态', () => {
  it('应在同批 entries 重渲后保持 per-path 展开态，折叠子树仍零 DOM 节点', () => {
    const entries = [dir('a'), dir('a/b'), dir('a/b/c'), file('a/b/c/d.ts')];
    const onFocus = vi.fn();
    const { rerender } = render(<FileTree entries={entries} focused={null} onFocus={onFocus} />);

    // `a/b/c`（3 段）缺省折叠 ⇒ 点它展开
    fireEvent.click(screen.getByLabelText('展开 a/b/c'));
    expect(rowOf('a/b/c/d.ts')).toBeDefined();

    rerender(<FileTree entries={[...entries]} focused={null} onFocus={onFocus} />);
    expect(rowOf('a/b/c/d.ts'), '重渲后 per-path 展开态保持').toBeDefined();

    // 收起一级目录 ⇒ 其子树整体离开 DOM
    fireEvent.click(screen.getByLabelText('收起 a'));
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(1);
    expect(rowOf('a/b')).toBeUndefined();
    expect(rowOf('a/b/c/d.ts')).toBeUndefined();
  });

  it('应在空条目时渲空树提示并零行节点', () => {
    renderTree([]);

    expect(screen.getByTestId('archi-file-tree-empty').textContent).toBe('仓库内无源文件');
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(0);
  });
});

describe('FileTree 图例与前端过滤纪律', () => {
  it('应常驻渲可折叠图例，展开后给出判定面说明逐字文案', () => {
    renderTree([file('src/a.ts')]);

    const legend = screen.getByTestId('archi-file-tree-legend');
    expect(within(legend).getByText('图例')).toBeTruthy();
    // 折叠态只有一行入口
    expect(legend.textContent).toBe('图例');

    fireEvent.click(within(legend).getByText('图例'));
    expect(legend.textContent).toContain(
      '仅显示判定面源文件（测试文件与架构配置 exclude 命中项不在树中；none 行 = 无元素认领）',
    );
    // 图例文本不影响树行渲染
    expect(rowOf('src/a.ts')).toBeDefined();
  });

  it('应对越过后端过滤的脏条目照常渲出（前端不做二次过滤）', () => {
    renderTree([
      file('src/a.test.tsx', { mark: 'member', owners: ['App.A'] }),
      file('src/b.rs_test'),
    ]);

    // 反向钉住：过滤面单点在后端，前端零二次过滤
    expect(rowOf('src/a.test.tsx')).toBeDefined();
    expect(rowOf('src/b.rs_test')).toBeDefined();
  });
});

describe('FileTree 入参越签名与规模边界', () => {
  it('应在 entries 为 undefined（越签名脏值）时抛错且不虚构条目', () => {
    const onFocus = vi.fn();
    const dirty = undefined as unknown as ArchiFileEntry[];
    expect(() => render(<FileTree entries={dirty} focused={null} onFocus={onFocus} />)).toThrow();
    expect(screen.queryAllByTestId('archi-file-node')).toHaveLength(0);
  });

  it('应在 1000 条扁平条目下渲染不抛错且 data-path 唯一', () => {
    const entries: ArchiFileEntry[] = [];
    for (let module = 0; module < 20; module += 1) {
      const root = `m${String(module).padStart(2, '0')}`;
      entries.push(dir(root), dir(`${root}/deep`), dir(`${root}/deep/inner`));
      for (let index = 0; index < 50; index += 1) {
        entries.push(
          file(`${root}/deep/inner/f${String(index).padStart(4, '0')}.ts`, {
            mark: 'member',
            owners: [`App.M${String(module).padStart(2, '0')}`],
          }),
        );
      }
    }
    renderTree(entries);

    const paths = renderedPaths();
    expect(paths.length).toBeGreaterThan(0);
    expect(new Set(paths).size).toBe(paths.length);
    // 默认只展开前 2 段 ⇒ 四段文件行缺省不在 DOM（可见行数有界）
    expect(paths.length).toBeLessThan(1000);
    expect(
      paths.every((path) => path.split('/').length <= 3),
      '可见行深度不超过 3 段',
    ).toBe(true);
    // 展开一个深层目录后其文件行进 DOM（标记与展开逻辑照常）
    fireEvent.click(screen.getByLabelText('展开 m00/deep/inner'));
    expect(rowOf('m00/deep/inner/f0000.ts')).toBeDefined();
    expect(markOf('m00/deep/inner/f0000.ts')?.getAttribute('data-mark')).toBe('member');
  });

  it('应对空 path / 超长 / emoji / 单段 path 形态分别给出可判行为', () => {
    const long = `src/${'长'.repeat(1100)}.ts`;
    renderTree([file(''), file(long), file('src/空 格🚀.ts'), file('single.ts')]);

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

describe('TreeModeSwitch 分段控件', () => {
  it('应渲两钮并以 aria-pressed 表达选中态，点击各发一次 onChange', () => {
    const onChange = vi.fn();
    render(<TreeModeSwitch mode="file" onChange={onChange} />);

    const group = screen.getByTestId('archi-tree-mode');
    expect(group.getAttribute('role')).toBe('group');
    const fileButton = group.querySelector<HTMLElement>('[data-mode="file"]');
    const elementButton = group.querySelector<HTMLElement>('[data-mode="element"]');
    expect(fileButton?.getAttribute('aria-pressed')).toBe('true');
    expect(elementButton?.getAttribute('aria-pressed')).toBe('false');

    fireEvent.click(elementButton!);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenLastCalledWith('element');

    fireEvent.click(fileButton!);
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenLastCalledWith('file');
  });

  it('应在 mode 变化后翻转两钮选中态且零内部 state 回写', () => {
    const onChange = vi.fn();
    const { rerender } = render(<TreeModeSwitch mode="file" onChange={onChange} />);
    rerender(<TreeModeSwitch mode="element" onChange={onChange} />);

    const group = screen.getByTestId('archi-tree-mode');
    expect(group.querySelector('[data-mode="file"]')?.getAttribute('aria-pressed')).toBe('false');
    expect(group.querySelector('[data-mode="element"]')?.getAttribute('aria-pressed')).toBe('true');
    expect(onChange).not.toHaveBeenCalled();
  });

  it('应与同屏 FileTree 相互独立（切换不改树的展开态与 onFocus 计数）', () => {
    const onChange = vi.fn();
    const onFocus = vi.fn();
    const entries = [dir('a'), dir('a/b'), dir('a/b/c'), file('a/b/c/d.ts')];
    const view = render(
      <>
        <TreeModeSwitch mode="file" onChange={onChange} />
        <FileTree entries={entries} focused={null} onFocus={onFocus} />
      </>,
    );

    fireEvent.click(screen.getByLabelText('展开 a/b/c'));
    expect(rowOf('a/b/c/d.ts')).toBeDefined();

    const elementButton = view.container.querySelector<HTMLElement>('[data-mode="element"]');
    fireEvent.click(elementButton!);
    expect(onChange).toHaveBeenCalledWith('element');
    expect(rowOf('a/b/c/d.ts'), '树的展开态不随分段切换变化').toBeDefined();
    expect(onFocus).not.toHaveBeenCalled();
  });
});
