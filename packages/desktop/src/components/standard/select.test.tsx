import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { StandardSelect } from './select';

// 零进程边界：不 mock 被测组件与 ui/** 底座（base-ui Select 真实渲染）；
// onChange 以 vi.fn() 注入观察面；弹层以 fireEvent.mouseDown 打开、
// findByRole('listbox') 异步收敛，option 经 pointerDown + click 选中。

const ITEMS = [
  { value: 'alpha', label: '甲档' },
  { value: 'beta', label: '乙档' },
];

type SelectOverrides = {
  value?: string | number | null;
  items?: { value: string | number; label: string }[];
  placeholder?: string;
  disabled?: boolean;
  onChange?: (value: string | number | null) => void;
};

/** 挂载 StandardSelect（data-testid 固定作 SelectValue 查询挂钩）。 */
function renderSelect(overrides: SelectOverrides = {}) {
  const {
    value = null,
    items = ITEMS,
    placeholder,
    disabled,
    onChange = vi.fn<(value: string | number | null) => void>(),
  } = overrides;
  const view = render(
    <StandardSelect
      value={value}
      items={items}
      onChange={onChange}
      placeholder={placeholder}
      disabled={disabled}
      data-testid="选中值锚"
    />,
  );
  return { onChange, view };
}

/** SelectValue 元素（data-testid 落点，承载占位 / label 与 data-value）。 */
function valueEl(): HTMLElement {
  return screen.getByTestId('选中值锚');
}

/** 触发器按钮（SelectValue 外层 role=combobox button）。 */
function triggerEl(): HTMLButtonElement {
  const button = valueEl().closest('button');
  if (!button) throw new Error('SelectValue 不在触发器按钮内');
  return button;
}

/** base-ui Select 以 mousedown 打开、frame 异步执行：以 listbox 出现收敛。 */
async function openSelect(): Promise<void> {
  fireEvent.mouseDown(triggerEl());
  await screen.findByRole('listbox');
}

/** 打开弹层并点选指定 label 项：pointerDown 先行满足鼠标选择守卫，click 提交选中。 */
async function pick(label: string): Promise<void> {
  await openSelect();
  const option = within(screen.getByRole('listbox')).getByRole('option', { name: label });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}

/** 当前弹层 option 文本清单。 */
function optionTexts(): string[] {
  return within(screen.getByRole('listbox'))
    .getAllByRole('option')
    .map((option) => option.textContent ?? '');
}

describe('StandardSelect：弹层清单与选中回报', () => {
  it('正向：mouseDown 打开弹层（findByRole("listbox") 收敛）：option 清单逐项等于 items labels', async () => {
    renderSelect();

    await openSelect();

    expect(optionTexts()).toEqual(['甲档', '乙档']);
  });

  it('正向：选中 option（pointerDown + click）→ onChange 恰收选中 value（string 档）', async () => {
    const { onChange } = renderSelect();

    await pick('乙档');

    expect(onChange).toHaveBeenCalledTimes(1);
    // base-ui onValueChange 直连携 eventDetails 第二入参：契约面取首入参 value
    expect(vi.mocked(onChange).mock.calls[0]?.[0]).toBe('beta');
  });

  it('正向：选中 value 为 number 的项 → onChange 恰收数值本体（number 档，非字符串化）', async () => {
    const { onChange } = renderSelect({
      items: [
        { value: 1, label: '一号' },
        { value: 2, label: '二号' },
      ],
    });

    await pick('二号');

    expect(onChange).toHaveBeenCalledTimes(1);
    // base-ui onValueChange 直连携 eventDetails 第二入参：契约面取首入参 value
    expect(vi.mocked(onChange).mock.calls[0]?.[0]).toBe(2);
  });

  it('正向：value 非 null（string 档）：触发器呈对应 label 且 data-value 在场等于选中值', () => {
    renderSelect({ value: 'alpha' });

    expect(valueEl().textContent).toBe('甲档');
    expect(valueEl().getAttribute('data-value')).toBe('alpha');
  });

  it('正向：data-testid 透出至 SelectValue，id / className 经 ...rest 同步透出', () => {
    render(
      <StandardSelect
        value="alpha"
        onChange={vi.fn<(value: string | number | null) => void>()}
        items={ITEMS}
        data-testid="自定义锚"
        id="选中值-id"
        className="自定-值类"
      />,
    );

    const anchor = screen.getByTestId('自定义锚');
    expect(anchor.textContent).toBe('甲档');
    expect(anchor.getAttribute('id')).toBe('选中值-id');
    expect(anchor.className).toContain('自定-值类');
  });
});

describe('StandardSelect：value null 档与 placeholder 双态', () => {
  it('异常：value=null（T | null 的 null 档）：触发器呈缺省占位「请选择」且 data-value 属性缺席', () => {
    renderSelect();

    expect(valueEl().textContent).toBe('请选择');
    expect(valueEl().getAttribute('data-value')).toBeNull();
  });

  it('边界：placeholder 逐档（自定义 / 空串 / 特殊字符 / 超长串 >1000 字符）在 value=null 档呈现，data-value 恒缺席', () => {
    const placeholders = ['挑一个', '', '\n🚀占位\0文本', `占${'位'.repeat(1001)}`];

    for (const placeholder of placeholders) {
      const { view } = renderSelect({ placeholder });
      expect(valueEl().textContent).toBe(placeholder);
      expect(valueEl().getAttribute('data-value')).toBeNull();
      view.unmount();
    }
  });
});

describe('StandardSelect：value 类型边界（字符串档 / 数值档）', () => {
  it('边界：value str 档（正常串 / 特殊字符 / 超长串 >1000 字符）→ data-value 字符串化在场且 label 呈现无损', () => {
    const specialValue = '\n🚀新\0值';
    const longValue = `值${'长'.repeat(1001)}`;
    const items = [
      { value: 'alpha', label: '甲档' },
      { value: specialValue, label: '特殊🚀档' },
      { value: longValue, label: '超长档' },
    ];

    for (const item of items) {
      const { view } = renderSelect({ value: item.value, items });
      expect(valueEl().textContent).toBe(item.label);
      expect(valueEl().getAttribute('data-value')).toBe(item.value);
      view.unmount();
    }
  });

  it('边界：value 空串 → data-value="" 属性在场；base-ui 以 serializeValue("")==="" 判未选中，触发器呈缺省占位（呈现与 null 档同形）', () => {
    renderSelect({ value: '', items: [{ value: '', label: '空串档' }] });

    const anchor = valueEl();
    expect(anchor.getAttribute('data-value')).toBe('');
    expect(anchor.textContent).toBe('请选择');
  });

  it('边界：value int 档（正数 / 0 / -1 / Number.MAX_SAFE_INTEGER）→ data-value 字符串化在场且 label 呈现正确', () => {
    const items = [
      { value: 42, label: '正数档' },
      { value: 0, label: '零档' },
      { value: -1, label: '负一档' },
      { value: Number.MAX_SAFE_INTEGER, label: '最大安全整数档' },
    ];

    for (const item of items) {
      const { view } = renderSelect({ value: item.value, items });
      expect(valueEl().textContent).toBe(item.label);
      expect(valueEl().getAttribute('data-value')).toBe(String(item.value));
      view.unmount();
    }
  });
});

describe('StandardSelect：items 与 label 边界', () => {
  it('异常：items=[]（空清单）：打开弹层零 option 且不抛错', async () => {
    renderSelect({ items: [] });

    await openSelect();

    expect(within(screen.getByRole('listbox')).queryAllByRole('option')).toHaveLength(0);
    expect(valueEl().textContent).toBe('请选择');
  });

  it('边界：items 逐档（单元素 / 多元素含 number 项 / 超大清单 1000 项）→ option 数量与 label 断言', async () => {
    const single = renderSelect({ items: [{ value: 'only', label: '唯一项' }] });
    await openSelect();
    expect(optionTexts()).toEqual(['唯一项']);
    single.view.unmount();

    const multi = renderSelect({
      items: [...ITEMS, { value: 3, label: '三号数值项' }],
    });
    await openSelect();
    expect(optionTexts()).toEqual(['甲档', '乙档', '三号数值项']);
    multi.view.unmount();

    const huge = renderSelect({
      items: Array.from({ length: 1000 }, (_, index) => ({
        value: index,
        label: `项${index}`,
      })),
    });
    await openSelect();
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(1000);
    expect(options[0].textContent).toBe('项0');
    expect(options[999].textContent).toBe('项999');
    huge.view.unmount();
  });

  it('边界：option label 逐档（空串 / 特殊字符含换行 emoji / 超长串 >1000 字符）呈现无损', async () => {
    const longLabel = `标${'长'.repeat(1001)}`;
    const labels = ['', '\n🚀行内标签', longLabel];
    renderSelect({ items: labels.map((label, index) => ({ value: `v${index}`, label })) });

    await openSelect();

    expect(optionTexts()).toEqual(labels);
  });
});

describe('StandardSelect：禁用与迭代改选', () => {
  it('异常：disabled=true → 触发器禁开（mouseDown 后 listbox 不出现）且 onChange 零调用', async () => {
    const { onChange } = renderSelect({ disabled: true });
    // base-ui useButton 对原生 button 落 disabled 原生属性
    expect(triggerEl().disabled).toBe(true);

    fireEvent.mouseDown(triggerEl());
    // 弹层开启按帧异步收敛：等一个宏任务拍后断言恒不出现
    await act(async () => {
      await new Promise((resolve) => {
        setTimeout(resolve, 50);
      });
    });

    expect(screen.queryByRole('listbox')).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('边界：连续改选 A → B（迭代）：onChange 逐次收新值，data-value 随受控 value 更新', async () => {
    const onChange = vi.fn<(value: string | number | null) => void>();
    const view = render(
      <StandardSelect value={null} onChange={onChange} items={ITEMS} data-testid="选中值锚" />,
    );

    await pick('甲档');
    expect(onChange).toHaveBeenCalledTimes(1);
    // base-ui onValueChange 直连携 eventDetails 第二入参：契约面取首入参 value
    expect(onChange.mock.calls[0]?.[0]).toBe('alpha');
    await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());

    view.rerender(
      <StandardSelect value="alpha" onChange={onChange} items={ITEMS} data-testid="选中值锚" />,
    );
    expect(valueEl().getAttribute('data-value')).toBe('alpha');
    expect(valueEl().textContent).toBe('甲档');

    await pick('乙档');
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange.mock.calls[1]?.[0]).toBe('beta');
    await waitFor(() => expect(screen.queryByRole('listbox')).toBeNull());

    view.rerender(
      <StandardSelect value="beta" onChange={onChange} items={ITEMS} data-testid="选中值锚" />,
    );
    expect(valueEl().getAttribute('data-value')).toBe('beta');
    expect(valueEl().textContent).toBe('乙档');
  });
});
