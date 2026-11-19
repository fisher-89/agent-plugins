import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { StandardCombobox } from './combobox';

// 零进程边界：不 mock 被测组件与 ui/combobox 底座（base-ui Combobox 真实渲染），
// onChange 以 vi.fn() 作被测 API 的显式入参注入；弹层以 fireEvent.mouseDown 打开、
// findByRole('listbox') 异步收敛，条目经 role=option 文本查询（不用类名）。

const ITEMS = ['deepseek-reasoner', 'deepseek-chat'];

type HarnessProps = {
  items: string[];
  initialValue?: string;
  disabled?: boolean;
  onChange: (value: string) => void;
};

/** 受控用法宿主：真实消费「输入文本即唯一真相」——onChange 记录后被写回 value。 */
function Harness({ items, initialValue = '', disabled = false, onChange }: HarnessProps) {
  const [value, setValue] = useState(initialValue);
  return (
    <StandardCombobox
      data-testid="标准组合框"
      disabled={disabled}
      items={items}
      onChange={(next) => {
        onChange(next);
        setValue(next);
      }}
      value={value}
    />
  );
}

function inputEl(): HTMLInputElement {
  return screen.getByTestId<HTMLInputElement>('标准组合框');
}

function type(value: string): void {
  fireEvent.change(inputEl(), { target: { value } });
}

/** 打开建议弹层（base-ui 以 input 上的 mousedown 打开，frame 异步执行）。 */
async function openList(): Promise<void> {
  fireEvent.mouseDown(inputEl());
  await screen.findByRole('listbox');
}

function optionTexts(): string[] {
  return within(screen.getByRole('listbox'))
    .getAllByRole('option')
    .map((option) => option.textContent ?? '');
}

describe('StandardCombobox：自由输入（建议不构成约束）', () => {
  it('逐字符键入目录外自定义值 → onChange 依次收到键入原文，末次为完整串且 data-value 同源', () => {
    const onChange = vi.fn<(value: string) => void>();
    render(<Harness items={ITEMS} onChange={onChange} />);
    const typed = 'my-custom-model';

    const prefixes: string[] = [];
    for (let index = 1; index <= typed.length; index += 1) {
      const prefix = typed.slice(0, index);
      prefixes.push(prefix);
      type(prefix);
      expect(inputEl().getAttribute('data-value'), `键入到 ${prefix} 时 data-value 同源`).toBe(
        prefix,
      );
    }

    expect(onChange.mock.calls.map(([value]) => value)).toEqual(prefixes);
    expect(onChange).toHaveBeenLastCalledWith('my-custom-model');
    expect(inputEl().value).toBe('my-custom-model');
  });

  it('键入目录外自定义值后关闭建议弹层 → 输入原文不被关层收尾回写清空', async () => {
    const onChange = vi.fn<(value: string) => void>();
    render(<Harness items={ITEMS} onChange={onChange} />);

    await openList();
    type('my-custom-model');
    fireEvent.keyDown(inputEl(), { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());

    expect(inputEl().value).toBe('my-custom-model');
    expect(inputEl().getAttribute('data-value')).toBe('my-custom-model');
    expect(onChange).toHaveBeenLastCalledWith('my-custom-model');
  });

  it('展开清单并选中建议项 → 输入框呈该原文、onChange 收到该值', async () => {
    const onChange = vi.fn<(value: string) => void>();
    render(<Harness items={ITEMS} onChange={onChange} />);

    await openList();
    expect(optionTexts()).toEqual(ITEMS);

    fireEvent.click(screen.getByRole('option', { name: 'deepseek-reasoner' }));

    await waitFor(() => expect(inputEl().value).toBe('deepseek-reasoner'));
    expect(inputEl().getAttribute('data-value')).toBe('deepseek-reasoner');
    expect(onChange).toHaveBeenLastCalledWith('deepseek-reasoner');
  });

  it('items 多于一项 → 展开清单逐项渲染（数量与原文逐一对应）', async () => {
    const items = ['alpha', 'beta', 'gamma', 'delta'];
    render(<Harness items={items} onChange={vi.fn()} />);

    await openList();

    const options = within(screen.getByRole('listbox')).getAllByRole('option');
    expect(options).toHaveLength(items.length);
    expect(options.map((option) => option.textContent)).toEqual(items);
  });
});

describe('StandardCombobox：空值语义（空串 = 留空）', () => {
  it('原值 low 后清空输入 → onChange("")、data-value 为空串，不回退到某项建议', () => {
    const onChange = vi.fn<(value: string) => void>();
    render(<Harness initialValue="low" items={ITEMS} onChange={onChange} />);
    expect(inputEl().value).toBe('low');

    type('');

    expect(inputEl().value).toBe('');
    expect(inputEl().getAttribute('data-value')).toBe('');
    expect(onChange).toHaveBeenLastCalledWith('');
  });

  it('value 为空串 → data-value 为空串、placeholder 缺省为「可输入或选择」', () => {
    render(<Harness items={ITEMS} onChange={vi.fn()} />);

    expect(inputEl().getAttribute('data-value')).toBe('');
    expect(inputEl().placeholder).toBe('可输入或选择');
  });

  it.each([
    ['超长串（>1000 chars）', 'm'.repeat(1200)],
    ['特殊字符（emoji / 内部空格）', '模型 🎉 v2 x'],
    ['不在 items 内的自定义值', 'not-in-catalog'],
  ])('value 为%s → 原样透出、零拦截', (_label, initialValue) => {
    render(<Harness initialValue={initialValue} items={ITEMS} onChange={vi.fn()} />);

    expect(inputEl().value).toBe(initialValue);
    expect(inputEl().getAttribute('data-value')).toBe(initialValue);
  });
});

describe('StandardCombobox：退化与禁用（边界）', () => {
  it('items 为空数组 → 聚焦键入后文档无 listbox、onChange 照常回调（退化纯输入框）', async () => {
    const onChange = vi.fn<(value: string) => void>();
    render(<Harness items={[]} onChange={onChange} />);

    fireEvent.mouseDown(inputEl());
    fireEvent.focus(inputEl());
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.queryByRole('listbox')).toBeNull();

    type('自定义模型');
    await waitFor(() => expect(onChange).toHaveBeenCalledWith('自定义模型'));
    expect(inputEl().getAttribute('data-value')).toBe('自定义模型');
  });

  it('disabled → 输入框禁用、按下不展开 listbox', async () => {
    render(<Harness disabled items={ITEMS} onChange={vi.fn()} />);

    expect(inputEl().disabled).toBe(true);

    fireEvent.mouseDown(inputEl());
    fireEvent.focus(inputEl());
    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.queryByRole('listbox')).toBeNull();
  });
});

describe('StandardCombobox：props 透传到输入框元素', () => {
  it('id / className / placeholder / data-testid 落在 input 元素上（非外层容器）', () => {
    render(
      <StandardCombobox
        className="custom-class"
        data-testid="标准组合框"
        id="组合框"
        items={ITEMS}
        onChange={vi.fn()}
        placeholder="自定义占位"
        value=""
      />,
    );

    const input = screen.getByTestId<HTMLInputElement>('标准组合框');
    expect(input.tagName).toBe('INPUT');
    expect(input.id).toBe('组合框');
    expect(input.placeholder).toBe('自定义占位');
    expect(input.className).toContain('custom-class');
    expect(input.getAttribute('data-value')).toBe('');
  });
});
