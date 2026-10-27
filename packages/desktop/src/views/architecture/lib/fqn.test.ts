import { describe, expect, it } from 'vite-plus/test';

import { shortName } from './fqn';

/**
 * `fqn.ts` 单测：`shortName(fqn)` 的末段取法单点——元素树节点与画布节点共用同一
 * 取法（同一元素在两处呈现同一名字），词汇外形态退化为「原样返回末段」且不抛错。
 * 纯函数零渲染、零 IPC、零环境垫片。
 */

/** 独立镜像实现：按 `.` 切段取末段（与源实现同口径，用于「单点取法」对拍）。 */
function lastSegment(fqn: string): string {
  const segments = fqn.split('.');
  return segments[segments.length - 1] ?? fqn;
}

describe('FQN 末段短名', () => {
  it('应取最后一个点之后的段（无点即自身，大小写逐字保留）', () => {
    expect(shortName('App.Auth.Token')).toBe('Token');
    expect(shortName('App.Auth')).toBe('Auth');
    expect(shortName('App')).toBe('App');
    expect(shortName('app.auth')).toBe('auth');
    expect(shortName('App.Auth.Token.Sink')).toBe('Sink');
  });

  it('应把点段的空形态逐字映射（不补默认名、不抛错）', () => {
    // 入参为空串 / 仅点 / 尾点 / 前导点 / 连续点：末段为空串或末段名
    const cases: Array<[string, string]> = [
      ['', ''],
      ['.', ''],
      ['App.', ''],
      ['.Token', 'Token'],
      ['App..Token', 'Token'],
      ['App.Auth.', ''],
    ];
    for (const [input, expected] of cases) {
      expect(() => shortName(input)).not.toThrow();
      expect(shortName(input)).toBe(expected);
    }
  });

  it('应逐字保留特殊字符（换行 / NUL / emoji / 全中文）', () => {
    expect(shortName('App.Auth\nToken')).toBe('Auth\nToken');
    expect(shortName('App.Auth\0Token')).toBe('Auth\0Token');
    expect(shortName('App.🚀Token')).toBe('🚀Token');
    expect(shortName('应用.认证.令牌')).toBe('令牌');
    expect(shortName('App.认证.🚀')).toBe('🚀');
  });

  it('应逐字保留超长入参（>1000 字符）与超长末段，不截断', () => {
    const longSegment = 'A'.repeat(1200);
    expect(shortName(`App.${longSegment}`)).toBe(longSegment);
    // 整体超长且无点 → 原样返回
    expect(shortName(longSegment)).toBe(longSegment);
    // 整体超长与多点前缀组合 → 仍只取末段
    const manyDots = `${'Seg.'.repeat(400)}Tail`;
    expect(shortName(manyDots)).toBe('Tail');
  });

  it('应在深链与规模面下只取末段（500 段）', () => {
    const deep = Array.from({ length: 500 }, (_, index) => `Seg${index}`).join('.');
    expect(shortName(deep)).toBe('Seg499');
    expect(shortName(`App.${deep}`)).toBe('Seg499');
  });

  it('应与独立镜像实现逐值一致（末段取法单点，结果内不含点）', () => {
    const samples = [
      'App',
      'App.Auth',
      'App.Auth.Token',
      'App.Auth.Token.Sink',
      'app.auth',
      '',
      '.',
      'App.',
      '.Token',
      'App..Token',
      '应用.认证.令牌',
      'App.🚀Token',
    ];
    for (const fqn of samples) {
      const name = shortName(fqn);
      expect(name).toBe(lastSegment(fqn));
      // 末段本身不含点（故再次取末段幂等）
      expect(name.includes('.')).toBe(false);
      expect(shortName(name)).toBe(name);
    }
  });

  it('应为纯函数：同入参连续调用等值且不抛错', () => {
    for (const fqn of ['App.Auth.Token', '', 'App.', 'App.🚀Token']) {
      expect(shortName(fqn)).toBe(shortName(fqn));
      expect(() => shortName(fqn)).not.toThrow();
    }
  });
});

// 说明：`fqn` 为 `string`（非 `string | null`），`null` / `undefined` 在类型层不可表示，
// 故不虚构该入参形态；`shortName` 的返回恒为 `string`（不返回 undefined）。
