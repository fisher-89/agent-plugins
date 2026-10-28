import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ArchiElement } from '../../../types/generated/bindings';
import { shortName } from '../lib/fqn';
import { ElementTree } from './element-tree';

function element(fqn: string, mark: ArchiElement['mark'] = 'none'): ArchiElement {
  return {
    fqn,
    kind: 'module',
    gate: 'off',
    path: [],
    summary: null,
    metadata: {},
    modelFile: '.dev-team/architecture/models/01-core.toml',
    modelLine: 1,
    mark,
  };
}

function renderTree(
  elements: ArchiElement[],
  focused: string | null = null,
): {
  onFocus: ReturnType<typeof vi.fn>;
} {
  const onFocus = vi.fn();
  render(<ElementTree elements={elements} focused={focused} onFocus={onFocus} />);
  return { onFocus };
}

/** 树节点 FQN 清单（渲染序）。 */
function renderedFqns(): string[] {
  return screen
    .queryAllByTestId('archi-tree-node')
    .map((node) => node.getAttribute('data-fqn') ?? '');
}

/** 单个树节点元素（按 FQN 定位；不在场返回 `undefined`）。 */
function nodeOf(fqn: string): HTMLElement | undefined {
  return screen
    .queryAllByTestId('archi-tree-node')
    .find((node) => node.getAttribute('data-fqn') === fqn);
}

/** 树节点标签文本（剥去状态点，仅剩短名 span 的文本，渲染序）。 */
function renderedLabels(): string[] {
  return screen.queryAllByTestId('archi-tree-node').map((node) => {
    const full = (node.textContent ?? '').trim();
    const mark = (node.querySelector('[data-testid="archi-node-mark"]')?.textContent ?? '').trim();
    return full.split(mark).join('').trim();
  });
}

afterEach(() => {
  cleanup();
});

describe('元素树层级派生', () => {
  it('应不受声明序影响：兄弟与顶层节点一律按 FQN 字典序呈现', () => {
    renderTree([element('App.Billing'), element('App'), element('App.Auth')]);
    expect(renderedFqns()).toEqual(['App', 'App.Auth', 'App.Billing']);
  });

  it('应为父节点渲染展开开关、为叶子节点留空位（不虚构可展开项）', () => {
    renderTree([element('App'), element('App.Auth'), element('App.Auth.Token')]);
    // 有子元素的节点才有展开按钮
    expect(screen.getByLabelText('收起 App')).toBeTruthy();
    expect(screen.getByLabelText('收起 App.Auth')).toBeTruthy();
    expect(screen.queryByLabelText('展开 App.Auth.Token')).toBeNull();
    expect(screen.queryByLabelText('收起 App.Auth.Token')).toBeNull();
  });

  it('应在元素集变化后重算树（新增子元素即上图，不因缓存停留旧形状）', () => {
    const { rerender } = render(
      <ElementTree
        elements={[element('App'), element('App.Auth')]}
        focused={null}
        onFocus={vi.fn()}
      />,
    );
    expect(renderedFqns()).toEqual(['App', 'App.Auth']);
    rerender(
      <ElementTree
        elements={[element('App'), element('App.Auth'), element('App.Auth.Token')]}
        focused={null}
        onFocus={vi.fn()}
      />,
    );
    expect(renderedFqns()).toEqual(['App', 'App.Auth', 'App.Auth.Token']);
  });

  it('应按 FQN 最长前缀派生三层树（父 = 最长已声明前缀）', () => {
    renderTree([
      element('App'),
      element('App.Auth'),
      element('App.Auth.Token'),
      element('App.Billing'),
    ]);
    // 顶层展开即含二层；三层需展开 App.Auth
    expect(renderedFqns()).toEqual(['App', 'App.Auth', 'App.Auth.Token', 'App.Billing']);

    const tree = screen.getByTestId('archi-element-tree');
    expect(within(tree).getByText('App')).toBeTruthy();
    expect(within(tree).getByText('Auth')).toBeTruthy();
    expect(within(tree).getByText('Token')).toBeTruthy();
    expect(within(tree).getByText('Billing')).toBeTruthy();
  });

  it('应挂到最长已声明前缀（中间层未声明时不丢节点、不造幽灵父级）', () => {
    renderTree([element('App'), element('App.Auth.Token')]);
    expect(renderedFqns()).toEqual(['App', 'App.Auth.Token']);
    // 树内短名取末段
    expect(within(screen.getByTestId('archi-element-tree')).getByText('Token')).toBeTruthy();
    // 未声明的中间层不产生节点
    expect(renderedFqns()).not.toContain('App.Auth');
  });

  it('应把名字相近的兄弟 FQN 当兄弟而非父子（. 边界切分）', () => {
    renderTree([element('App'), element('App.Auth'), element('App.AuthLegacy')]);
    expect(renderedFqns()).toEqual(['App', 'App.Auth', 'App.AuthLegacy']);
    const tree = screen.getByTestId('archi-element-tree');
    // Auth 无子元素 → 无可展开按钮；AuthLegacy 与 Auth 同级
    expect(within(tree).getByText('Auth')).toBeTruthy();
    expect(within(tree).getByText('AuthLegacy')).toBeTruthy();
  });

  it('应在无顶层元素时退化为扁平列表且不丢节点', () => {
    renderTree([element('App.Auth'), element('App.Auth.Token')]);
    expect(renderedFqns()).toEqual(['App.Auth', 'App.Auth.Token']);
  });
});

describe('节点文案（同一元素同一名字）', () => {
  it('应按末段短名呈现标签（与画布节点同源，fqn 全文不进 label）', () => {
    const elements = [element('App'), element('App.Auth'), element('App.Auth.Token')];
    renderTree(elements);

    expect(renderedLabels()).toEqual(['App', 'Auth', 'Token']);
    // 与 `lib/fqn` 的 `shortName` 单点取法逐值相同（元素树与画布节点不各写一份取法）
    expect(renderedLabels()).toEqual(elements.map((item) => shortName(item.fqn)));

    // 全 FQN 只在 title 上承载，不进 label
    const token = nodeOf('App.Auth.Token');
    expect(token?.getAttribute('title')).toBe('App.Auth.Token');
    expect(token?.textContent ?? '').not.toContain('App.');
  });

  it('应在点段空形态下呈现空标签且不抛错（不补默认名）', () => {
    expect(() => renderTree([element('App.')])).not.toThrow();
    const node = nodeOf('App.');
    expect(node).toBeTruthy();
    // 末段为空串 → 标签为空串（不补「App」一类默认名），状态点仍在
    expect(renderedLabels()).toEqual(['']);
    expect(node?.textContent ?? '').not.toContain('App');
    expect(node?.querySelector('[data-testid="archi-node-mark"]')?.textContent).toBe('⚪');
  });
});

describe('节点状态点', () => {
  it('应按词汇渲染 ⚪ / 🟡 并在词汇外退化为 ⚪', () => {
    renderTree([
      element('App', 'none'),
      element('App.Auth', 'unfulfilled'),
      // 词汇外值（类型层不可表示，运行期容错面）
      { ...element('App.Billing'), mark: 'unknown' as unknown as ArchiElement['mark'] },
    ]);
    const marks = screen
      .queryAllByTestId('archi-node-mark')
      .map((node) => (node.textContent ?? '').trim());
    expect(marks).toEqual(['⚪', '🟡', '⚪']);
  });
});

describe('停摆与空态', () => {
  it('应在空元素集下零节点并呈现空态文案', () => {
    renderTree([]);
    expect(screen.queryAllByTestId('archi-tree-node')).toHaveLength(0);
    expect(screen.getByTestId('archi-tree-empty')).toBeTruthy();
  });
});

describe('交互', () => {
  it('应在点击节点时上抛聚焦 FQN，展开 / 收起不改变节点数据', () => {
    const { onFocus } = renderTree([
      element('App'),
      element('App.Auth'),
      element('App.Auth.Token'),
    ]);
    const tokenNode = screen
      .queryAllByTestId('archi-tree-node')
      .find((node) => node.getAttribute('data-fqn') === 'App.Auth.Token');
    expect(tokenNode).toBeTruthy();
    fireEvent.click(tokenNode!);
    expect(onFocus).toHaveBeenCalledWith('App.Auth.Token');

    const toggle = screen.getByLabelText('收起 App.Auth');
    fireEvent.click(toggle);
    expect(renderedFqns()).toEqual(['App', 'App.Auth']);
    fireEvent.click(screen.getByLabelText('展开 App.Auth'));
    expect(renderedFqns()).toEqual(['App', 'App.Auth', 'App.Auth.Token']);
  });
});

describe('规模与特殊字符', () => {
  it('应在 500 节点与深链下不丢节点（超长 / emoji FQN 逐字呈现）', () => {
    const long = `App.${'长'.repeat(1100)}`;
    const elements: ArchiElement[] = [element('App')];
    for (let index = 0; index < 498; index += 1) {
      elements.push(element(`App.Mod${String(index).padStart(3, '0')}`));
    }
    elements.push(element('App.🚀Auth'));
    elements.push(element(long));

    renderTree(elements);
    const fqns = renderedFqns();
    expect(fqns).toContain('App.🚀Auth');
    expect(fqns).toContain(long);
    expect(fqns.length).toBe(elements.length);
  });
});
