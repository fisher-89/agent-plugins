// @vitest-environment jsdom
import { render, renderHook, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import {
  ConfigApplyScope,
  type ConfigApplyActions,
  useConfigApplyActions,
} from './config-apply-scope';

// ---------------------------------------------------------------------------
// 纯 React context：以 `renderHook` / 最小消费组件真实渲染；零 IPC 面（本文件
// 不触 invoke，故不 mock `@tauri-apps/api/core`）。
// ---------------------------------------------------------------------------

/** 最小消费组件：把读到的动作面摊成可断言的文本。 */
function Probe(): React.JSX.Element {
  const actions = useConfigApplyActions();
  if (actions === null) return <div data-testid="probe">null</div>;
  return <div data-testid="probe">{`${actions.root}`}</div>;
}

/** 以 config-apply-scope 的 provider 包裹探针渲染。 */
function renderWithScope(value: ConfigApplyActions, children?: React.ReactNode) {
  return render(<ConfigApplyScope value={value}>{children ?? <Probe />}</ConfigApplyScope>);
}

describe('ConfigApplyScope：动作面传递', () => {
  it('包裹子组件后读到的 {root, onApplied} 与传入 value 逐字段相等（同引用）', () => {
    const onApplied = vi.fn();
    const value: ConfigApplyActions = { root: 'C:\\demo\\alpha', onApplied };

    let captured: ConfigApplyActions | null = null;
    function Capture(): null {
      captured = useConfigApplyActions();
      return null;
    }
    renderWithScope(value, <Capture />);

    expect(captured).not.toBeNull();
    expect(captured).toBe(value);
    expect(captured!.root).toBe('C:\\demo\\alpha');
    expect(captured!.onApplied).toBe(onApplied);
  });

  it('无 provider → 返回 null（缺席降级，不抛错）', () => {
    const { result } = renderHook(() => useConfigApplyActions());
    expect(result.current).toBeNull();

    render(<Probe />);
    expect(screen.getByTestId('probe').textContent).toBe('null');
  });

  it('value 换根（root 变化）→ 消费侧读到新值（不缓存旧引用）', () => {
    const first: ConfigApplyActions = { root: 'C:\\demo\\alpha', onApplied: vi.fn() };
    const second: ConfigApplyActions = { root: 'C:\\demo\\beta', onApplied: vi.fn() };

    const { rerender } = renderWithScope(first);
    expect(screen.getByTestId('probe').textContent).toBe('C:\\demo\\alpha');

    rerender(
      <ConfigApplyScope value={second}>
        <Probe />
      </ConfigApplyScope>,
    );
    expect(screen.getByTestId('probe').textContent).toBe('C:\\demo\\beta');
  });
});

describe('ConfigApplyScope：子节点形态与嵌套', () => {
  it.each([
    ['单元素', <Probe key="single" />],
    [
      'fragment',
      <>
        <Probe />
        <span data-testid="sibling">旁支</span>
      </>,
    ],
    ['零子节点', undefined],
  ])('子节点形态 %s → 渲染零崩且 context 仍可读', (_label, children) => {
    const value: ConfigApplyActions = { root: 'C:\\demo\\alpha', onApplied: vi.fn() };
    renderWithScope(value, children);
    // 零子节点形态无探针，只断言渲染未抛错（上面 render 已成功）
    if (children !== undefined) {
      expect(screen.getByTestId('probe').textContent).toBe('C:\\demo\\alpha');
    }
  });

  it('嵌套两层 provider → 内层覆盖外层（最近者优先），onApplied 取内层回调', () => {
    const outerApplied = vi.fn();
    const innerApplied = vi.fn();
    let captured: ConfigApplyActions | null = null;
    function Capture(): null {
      captured = useConfigApplyActions();
      return null;
    }

    render(
      <ConfigApplyScope value={{ root: '外层根', onApplied: outerApplied }}>
        <ConfigApplyScope value={{ root: '内层根', onApplied: innerApplied }}>
          <Capture />
        </ConfigApplyScope>
      </ConfigApplyScope>,
    );

    expect(captured).not.toBeNull();
    expect(captured!.root).toBe('内层根');
    expect(captured!.onApplied).toBe(innerApplied);
    expect(captured!.onApplied).not.toBe(outerApplied);
  });
});
