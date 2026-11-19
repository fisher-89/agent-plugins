import { describe, expect, it } from 'vite-plus/test';

import { formatTokens } from './token';

describe('formatTokens：token 缩写分级（D8 阶界）', () => {
  it('阶下原样与 k 阶：999 → "999"（< 1000 原样十进制）、1_000 → "1k"、12_000 → "12k"', () => {
    expect(formatTokens(999)).toBe('999');
    expect(formatTokens(1_000)).toBe('1k');
    expect(formatTokens(12_000)).toBe('12k');
  });

  it('一位小数四舍五入与去尾零：12_300 → "12.3k"、12_360 → "12.4k"、12_000 → "12k" 非 "12.0k"', () => {
    expect(formatTokens(12_300)).toBe('12.3k');
    expect(formatTokens(12_360)).toBe('12.4k');
    expect(formatTokens(12_000)).toBe('12k');
    expect(formatTokens(12_000)).not.toBe('12.0k');
  });

  it('M 阶：1_000_000 → "1M"、1_500_000 → "1.5M"、2_300_000 → "2.3M"', () => {
    expect(formatTokens(1_000_000)).toBe('1M');
    expect(formatTokens(1_500_000)).toBe('1.5M');
    expect(formatTokens(2_300_000)).toBe('2.3M');
  });

  it('null & 0 → "—"（em dash，与耗时缺席 "-" 词面分立锚）', () => {
    expect(formatTokens(null)).toBe('—');
    expect(formatTokens(null)).not.toBe('-');
  });

  it('0 → "0"（最小值原样，不落 "0k"）', () => {
    expect(formatTokens(0)).toBe('0');
  });

  it('999_999 → "1000k"（k 阶封顶不跨界 M，D8 边界留痕）', () => {
    expect(formatTokens(999_999)).toBe('1000k');
  });

  it('超大值（9_999_999_999 级）→ M 阶输出不炸不跨界（D8 无 B 阶）', () => {
    expect(() => formatTokens(9_999_999_999)).not.toThrow();
    const label = formatTokens(9_999_999_999);
    expect(label.endsWith('M')).toBe(true);
    expect(label).not.toContain('B');
  });

  it('NaN → 不抛错、返回可读串（无抛错路径，非法数值防御）', () => {
    let label: string | undefined;
    expect(() => {
      label = formatTokens(Number.NaN);
    }).not.toThrow();
    expect(typeof label).toBe('string');
    expect(label!.length).toBeGreaterThan(0);
  });
});
