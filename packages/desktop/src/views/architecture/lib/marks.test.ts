import { describe, expect, it } from 'vite-plus/test';

import { edgeVisual } from './marks';

/**
 * `marks.ts` 单测（AC-22）：边四值 / 节点两值的确定性样式、词汇外退化与零标记
 * 基线。纯函数零渲染、零 IPC、零环境垫片。
 */

/** 边标记词汇（封闭四值；与 Rust 侧 `EdgeMark` 词表手工同源）。 */
const EDGE_MARKS = ['none', 'unfulfilled', 'crossKind', 'exempt'] as const;

describe('边标记视觉映射', () => {
  it('应逐值为四类边给出确定性样式（none 实线 / 灰虚线 / 橙虚线 / 灰实线）', () => {
    expect(edgeVisual('none')).toEqual({ stroke: 'var(--chart-2)', dasharray: null });
    expect(edgeVisual('unfulfilled')).toEqual({
      stroke: 'var(--muted-foreground)',
      dasharray: '6 4',
    });
    expect(edgeVisual('crossKind')).toEqual({ stroke: 'var(--warn)', dasharray: '4 3' });
    expect(edgeVisual('exempt')).toEqual({ stroke: 'var(--muted-foreground)', dasharray: null });
  });

  it('应在词汇外值上退化为与 none 同款中性样式且不抛错', () => {
    const neutral = edgeVisual('none');
    for (const mark of ['', 'unknown', 'None', 'crosskind', 'UNFULFILLED ']) {
      expect(() => edgeVisual(mark)).not.toThrow();
      expect(edgeVisual(mark)).toEqual(neutral);
    }
  });

  it('应恒定返回两字段对象且同入参结果等值（纯函数）', () => {
    for (const mark of EDGE_MARKS) {
      const visual = edgeVisual(mark);
      expect(Object.keys(visual).sort()).toEqual(['dasharray', 'stroke']);
      expect(typeof visual.stroke).toBe('string');
      expect(visual.dasharray === null || typeof visual.dasharray === 'string').toBe(true);
      expect(edgeVisual(mark)).toEqual(visual);
    }
  });
});
