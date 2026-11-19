import { describe, expect, it } from 'vite-plus/test';

import { runningDurationLabel, settledDurationLabel } from './metrics';

// ---------------------------------------------------------------------------
// metrics 纯函数直测（desktop-step-running-snapshot / D8 / D10）：零 mock——
// `formatDurationMs` 模块私有不导出（knip test-only export 纪律），分级断言
// 全部经两个导出标签函数（settled / running）覆盖；now 参数注入（running）
// 零 Date.now() 依赖。缺席 / 0ms 判定收本模块单点：`startedAt <= 0` /
// `endedAt <= startedAt` 口径逐用例钉死，调用侧无 0 魔数散落（R4）。
// ---------------------------------------------------------------------------

describe('settledDurationLabel：已收口步耗时段（缺席判定单点）', () => {
  it('分秒级分级互斥：(1_000_000, 1_200_000) 差 200_000ms → "3m20s"（不落秒小数）', () => {
    expect(settledDurationLabel(1_000_000, 1_200_000)).toBe('3m20s');
  });

  it('秒级一位小数：(1_000_000, 1_001_234) → "1.2s"、(1_000_000, 1_059_900) → "59.9s"', () => {
    expect(settledDurationLabel(1_000_000, 1_001_234)).toBe('1.2s');
    expect(settledDurationLabel(1_000_000, 1_059_900)).toBe('59.9s');
  });

  it('时级两位补零分：差 3_600_000 → "1h00m"、差 3_900_000 → "1h05m"、差 7_200_000 → "2h00m"', () => {
    expect(settledDurationLabel(1_000_000, 1_000_000 + 3_600_000)).toBe('1h00m');
    expect(settledDurationLabel(1_000_000, 1_000_000 + 3_900_000)).toBe('1h05m');
    expect(settledDurationLabel(1_000_000, 1_000_000 + 7_200_000)).toBe('2h00m');
  });

  it('999ms → "1.0s"（秒阶边界留痕，D8）', () => {
    expect(settledDurationLabel(1_000_000, 1_000_999)).toBe('1.0s');
  });

  it('ended == started（0ms 瞬步）→ null（省显——调用侧不渲染该段）', () => {
    expect(settledDurationLabel(1_000_000, 1_000_000)).toBeNull();
  });

  it('ended < started（逆序 / 容差）→ null（`<=` 判定省显）', () => {
    expect(settledDurationLabel(1_000_000, 999_999)).toBeNull();
  });

  it('startedAt = 0（旧库行缺省）→ "-"；startedAt < 0 → "-"（缺席判定单点，R4）', () => {
    expect(settledDurationLabel(0, 1_200_000)).toBe('-');
    expect(settledDurationLabel(-1, 1_200_000)).toBe('-');
  });

  it('endedAt = null（running / 库读史残留）→ "-"', () => {
    expect(settledDurationLabel(1_000_000, null)).toBe('-');
  });

  it('startedAt / endedAt 含 NaN → 不抛错（缺席或原样分级，无抛错路径）', () => {
    expect(() => settledDurationLabel(Number.NaN, 1_200_000)).not.toThrow();
    expect(() => settledDurationLabel(1_000_000, Number.NaN)).not.toThrow();
    // endedAt = NaN 非 null 且不满足 <= 判定（NaN 比较）→ 落分级臂但不炸
    expect(typeof settledDurationLabel(1_000_000, Number.NaN)).toBe('string');
  });
});

describe('runningDurationLabel：running 走秒段（now 参数注入纯函数）', () => {
  it('秒级：(1_000_000, 1_001_234) → "1.2s"（零 Date.now() 依赖）', () => {
    expect(runningDurationLabel(1_000_000, 1_001_234)).toBe('1.2s');
  });

  it('时阶界：(1, 3_600_000) 差 3_599_999 → "59m59s"；(1, 3_600_001) → "1h00m"', () => {
    expect(runningDurationLabel(1, 3_600_000)).toBe('59m59s');
    expect(runningDurationLabel(1, 3_600_001)).toBe('1h00m');
  });

  it('长跑形态：差 7_200_000 → "2h00m"', () => {
    expect(runningDurationLabel(1, 1 + 7_200_000)).toBe('2h00m');
  });

  it('startedAt = 0 → "-"；startedAt < 0 → "-"（缺席口径与 settled 同源单点）', () => {
    expect(runningDurationLabel(0, 1_001_234)).toBe('-');
    expect(runningDurationLabel(-1, 1_001_234)).toBe('-');
  });

  it('now == startedAt（刚起步 0ms）→ "0.0s"（running 恒显、无省显路径）', () => {
    expect(runningDurationLabel(1_000_000, 1_000_000)).toBe('0.0s');
  });

  it('now < startedAt（时钟回拨形态）→ 不抛错、非空串输出（负差防御）', () => {
    let label: string | undefined;
    expect(() => {
      label = runningDurationLabel(1_000_000, 999_999);
    }).not.toThrow();
    expect(typeof label).toBe('string');
    expect(label!.length).toBeGreaterThan(0);
  });
});
