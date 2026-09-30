// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ConfigExtraField } from '../../../types/generated/bindings';
import { ExtraFieldsSection } from './extra-fields-section';

// ---------------------------------------------------------------------------
// ExtraFieldsSection 单测（AC-5）：纯组件 fixture 直传入参渲染断言，无 mock。
// JSON 预览（JSON.stringify(value, null, 2)）的 passthrough 可见性即需求本体，
// 以「预览文本 == 原文序列化」逐字断言。
// ---------------------------------------------------------------------------

describe('ExtraFieldsSection：未知字段 passthrough JSON 预览（AC-5）', () => {
  it('逐条 key + JSON 预览呈现（config-extra 在场），对象 / 数组 / 标量 / null 多形态均可预览', () => {
    const extra: ConfigExtraField[] = [
      { key: 'customObject', value: { nested: { deep: [1, 2, 3] } } },
      { key: 'customArray', value: ['a', 'b'] },
      { key: 'customScalar', value: 42 },
      { key: 'customText', value: 'hello' },
      { key: 'customNull', value: null },
    ];

    render(<ExtraFieldsSection extra={extra} />);

    expect(screen.getByTestId('config-extra') !== null).toBe(true);
    // 预览为多行等宽块：以 textContent 与原文序列化逐字对照（passthrough 可见性）
    const previews = Array.from(screen.getByTestId('config-extra').querySelectorAll('pre'));
    expect(previews).toHaveLength(extra.length);
    for (const [index, field] of extra.entries()) {
      expect(screen.getByText(field.key) !== null).toBe(true);
      expect(previews[index].textContent).toBe(JSON.stringify(field.value, null, 2));
    }
  });

  it('空数组：整区不渲染（config-extra 不在场）', () => {
    const { container } = render(<ExtraFieldsSection extra={[]} />);

    expect(screen.queryByTestId('config-extra')).toBeNull();
    expect(container.textContent).toBe('');
  });

  it('深嵌套 / 超长 JSON 值预览完整无损（key → 原文值不变）', () => {
    const deep = { a: { b: { c: { d: [1, 2, { e: true }] } } } };
    const longText = '长'.repeat(1200);
    const extra: ConfigExtraField[] = [
      { key: 'deep', value: deep },
      { key: 'long', value: longText },
    ];

    render(<ExtraFieldsSection extra={extra} />);

    expect(screen.getByTestId('config-extra') !== null).toBe(true);
    expect(screen.getByText('deep') !== null).toBe(true);
    expect(screen.getByText('long') !== null).toBe(true);
    const previews = Array.from(screen.getByTestId('config-extra').querySelectorAll('pre'));
    expect(previews.map((pre) => pre.textContent)).toEqual([
      JSON.stringify(deep, null, 2),
      JSON.stringify(longText, null, 2),
    ]);
  });
});
