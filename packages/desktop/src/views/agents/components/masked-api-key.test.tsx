// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vite-plus/test';

import { MaskedApiKey } from './masked-api-key';

// MaskedApiKey 为纯展示组件（无进程边界无 mock）：遮蔽口径（末 3 字符、
// 空 / 过短恒 sk-***）为 design 语义约束，形态断言经 data-testid 取文本。

/** 渲染单实例并读取遮蔽文本（用例内逐一 unmount，避免多实例干扰断言）。 */
function maskedTextOf(apiKey: string): string {
  const view = render(<MaskedApiKey apiKey={apiKey} />);
  const text = screen.getByTestId('masked-api-key').textContent ?? '';
  view.unmount();
  return text;
}

afterEach(() => {
  cleanup();
});

describe('MaskedApiKey：遮蔽形态（AC-9）', () => {
  it('常规 key → sk-*** 前缀 + 末 3 字符（sk-***abc 形态）；渲染输出不含明文全文', () => {
    expect(maskedTextOf('sk-live-abcdefgh')).toBe('sk-***fgh');
    expect(maskedTextOf('1234567890')).toBe('sk-***890');

    // 渲染输出不含明文全文（全文仅在遮蔽形态下露出末 3 字符）
    const rendered = maskedTextOf('sk-live-abcdefgh');
    expect(rendered).not.toContain('sk-live-abcdefgh');
    expect(rendered.startsWith('sk-***')).toBe(true);
  });

  it('恰 4 字符（可遮蔽最短形态）→ sk-*** + 末 3 字符', () => {
    expect(maskedTextOf('abcd')).toBe('sk-***bcd');
  });
});

describe('MaskedApiKey：空 / 过短恒 sk-***（边界）', () => {
  it('空串 → 恒 sk-***（不回显原值）', () => {
    expect(maskedTextOf('')).toBe('sk-***');
  });

  it('长度 ≤3（含恰 3 字符）→ 恒 sk-***（防 sk-***abc 恰为原文泄露）', () => {
    expect(maskedTextOf('a')).toBe('sk-***');
    expect(maskedTextOf('ab')).toBe('sk-***');
    // 恰 3 字符：若回显末 3 字符则 sk-***abc 恰为原文——全遮蔽兜底
    expect(maskedTextOf('abc')).toBe('sk-***');
    expect(maskedTextOf('abc')).not.toContain('abc');
  });
});

describe('MaskedApiKey：超长与多字节 key（边界）', () => {
  it('超长（>1000 字符）key 末 3 字符截取不悬挂', () => {
    const longKey = `${'k'.repeat(1000)}xyz`;
    expect(maskedTextOf(longKey)).toBe('sk-***xyz');
  });

  it('emoji / 多字节 key 末 3 字符截取不抛错不崩（slice 按 UTF-16 码元粒度，与前端展示层同口径）', () => {
    // 末位 emoji（完整代理对落于末 3 码元内）：末 3 码元 = 乙 + 🎉 代理对，
    // 不产生半个字符
    expect(maskedTextOf('前缀甲乙🎉')).toBe('sk-***乙🎉');
    // 全 emoji key：与 JS string slice 语义逐字一致（锁定实现粒度，不以
    // 「字符数」臆断——Unicode 字形簇与码元粒度的取舍属实现可微调项）
    expect(maskedTextOf('🎉🎉🎉🎉')).toBe(`sk-***${'🎉🎉🎉🎉'.slice(-3)}`);
  });
});
