import { describe, expect, it } from 'vite-plus/test';

import { isStuckToBottom, normalizeCarriageReturns } from './stdout-text';

// ---------------------------------------------------------------------------
// stdout-text 两条纯判据直测（change-detail-live-stdout / AC-8）：零 mock——两个
// 函数零依赖（零 DOM / 零 IPC / 零 I/O），用例直接以字面量入参断言返回值。
// 跨模块生产消费点由 `stdout-panel.tsx` 承担（knip 判据因此不违规）。
// ---------------------------------------------------------------------------

describe('normalizeCarriageReturns：逐行只留最后一个 \\r 之后的内容', () => {
  it('正向：多行 cargo 进度序列 → 逐行取最后一段', () => {
    const input = 'Compiling a\rCompiling b\rCompiling c\nCompiling d\rCompiling e';
    expect(normalizeCarriageReturns(input)).toBe('Compiling c\nCompiling e');
  });

  it('正向：无 \\r 的多行文本逐字返回（恒等变换，含 emoji / 中文 / 替换符）', () => {
    const input = '行一 🚀\n替换符 \uFFFD 在此\nplain line\n';
    expect(normalizeCarriageReturns(input)).toBe(input);
  });

  it('异常：替换符 / \\0 / 制表符原样保留（归一不触碰其它字符）', () => {
    const input = 'a\u0000b\tc\uFFFDd\rlast';
    expect(normalizeCarriageReturns(input)).toBe('last');
    const untouched = 'a\u0000b\tc\uFFFDd';
    expect(normalizeCarriageReturns(untouched)).toBe(untouched);
  });

  it.each([
    ['空串', '', ''],
    ['单 \\r', '\r', ''],
    // `\r\n` 拆行为 `["\r", ""]` ⇒ 逐行归一为 `["", ""]` ⇒ join 得单个 `\n`
    //（实现逐行独立归一、不吞换行符本身）
    ['\\r\\n', '\r\n', '\n'],
    ['"a\\r"', 'a\r', ''],
  ])('边界：%s → 归一后为该值', (_label, input, expected) => {
    expect(normalizeCarriageReturns(input)).toBe(expected);
  });

  it.each([
    ['行首 CR', '\ra\n\rb', 'a\nb'],
    ['连续 CR', 'x\r\r y', ' y'],
  ])('边界：%s 只留最后一个 \\r 之后', (_label, input, expected) => {
    expect(normalizeCarriageReturns(input)).toBe(expected);
  });

  it('边界：单行 >10000 chars 含 20 个 CR → 只保留最后一段，零截断', () => {
    const segments: string[] = [];
    for (let index = 0; index < 20; index += 1) segments.push(`seg-${index}-${'x'.repeat(500)}`);
    const input = segments.join('\r');
    const output = normalizeCarriageReturns(input);
    expect(input.length).toBeGreaterThan(10_000);
    expect(output).toBe(segments[segments.length - 1]);
  });

  it('边界：100 行 × 每行 1 个 CR → 每行只剩尾部，行数不变', () => {
    const input = Array.from(
      { length: 100 },
      (_unused, index) => `head-${index}\rtail-${index}`,
    ).join('\n');
    const output = normalizeCarriageReturns(input);
    const lines = output.split('\n');
    expect(lines).toHaveLength(100);
    expect(lines[0]).toBe('tail-0');
    expect(lines[99]).toBe('tail-99');
  });

  it('边界：幂等性 f(f(x)) === f(x)（抽样四形态）', () => {
    const samples = ['Compiling a\rCompiling b\nnext', '', '\r', 'a\r\r y\n\uFFFD'];
    for (const sample of samples) {
      const once = normalizeCarriageReturns(sample);
      expect(normalizeCarriageReturns(once)).toBe(once);
    }
  });
});

describe('isStuckToBottom：粘底判定（容差 1 px）', () => {
  it('正向：恰在底部（差值 0）→ true', () => {
    expect(isStuckToBottom({ scrollTop: 900, scrollHeight: 1000, clientHeight: 100 })).toBe(true);
  });

  it.each([
    ['顶部', 0],
    ['中途', 450],
  ])('正向：%s → false', (_label, scrollTop) => {
    expect(isStuckToBottom({ scrollTop, scrollHeight: 1000, clientHeight: 100 })).toBe(false);
  });

  it.each([
    ['scrollTop 越界（负差值）', { scrollTop: 1000, scrollHeight: 1000, clientHeight: 100 }, true],
    // 负 scrollTop 使差值大于容差 ⇒ 判为「未在底部」（`<= 1` 逐值成立，零钳制）
    ['scrollTop 为负数', { scrollTop: -50, scrollHeight: 1000, clientHeight: 100 }, false],
  ])('异常：%s → %s（判据符号自然成立，零 panic 零钳制）', (_label, metrics, expected) => {
    expect(isStuckToBottom(metrics)).toBe(expected);
  });

  it.each([
    ['差值恰 0', 900, true],
    ['差值恰 1', 899, true],
    ['差值 1.0001', 898.9999, false],
  ])('边界：%s → %s（1 px 容差分界）', (_label, scrollTop, expected) => {
    expect(isStuckToBottom({ scrollTop, scrollHeight: 1000, clientHeight: 100 })).toBe(expected);
  });

  it.each([
    ['全零尺寸', { scrollTop: 0, scrollHeight: 0, clientHeight: 0 }, true],
    ['clientHeight 0 且内容极高', { scrollTop: 0, scrollHeight: 5000, clientHeight: 0 }, false],
    ['任一维 NaN', { scrollTop: Number.NaN, scrollHeight: 1000, clientHeight: 100 }, false],
  ])('边界：%s → %s（零 throw）', (_label, metrics, expected) => {
    expect(isStuckToBottom(metrics)).toBe(expected);
  });

  it('边界：超大数值按差值判定、无精度炸裂不四舍五入', () => {
    const big = Number.MAX_SAFE_INTEGER;
    expect(isStuckToBottom({ scrollTop: big, scrollHeight: big, clientHeight: 0 })).toBe(true);
    expect(isStuckToBottom({ scrollTop: 0, scrollHeight: big, clientHeight: 100 })).toBe(false);
  });
});
