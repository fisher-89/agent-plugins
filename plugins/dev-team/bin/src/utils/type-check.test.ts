/**
 * 单元测试: utils/type-check.ts — isPlainObject
 *
 * 覆盖 isPlainObject 的类型守卫行为：
 * - 普通对象 / Object.create(null) 为 true
 * - null、数组、原始类型为 false
 * - 模块导出可从 utils 入口导入
 *
 * @see plugins/dev-team/bin/src/utils/type-check.ts
 */

import { describe, it, expect } from 'vite-plus/test';

import { isPlainObject } from './type-check';

// ===========================================================================
// isPlainObject — 正向：应识别为对象
// ===========================================================================

describe('isPlainObject — 正向', () => {
  it('空对象应返回 true', () => {
    expect(isPlainObject({})).toBe(true);
  });

  it('带键值的普通对象应返回 true', () => {
    expect(isPlainObject({ a: 1, b: 'x' })).toBe(true);
  });

  it('Object.create(null) 应返回 true', () => {
    expect(isPlainObject(Object.create(null))).toBe(true);
  });

  it('嵌套对象应返回 true', () => {
    expect(isPlainObject({ nested: { ok: true } })).toBe(true);
  });
});

// ===========================================================================
// isPlainObject — 反向：null / 数组 / 原始类型
// ===========================================================================

describe('isPlainObject — 反向', () => {
  it('null 应返回 false', () => {
    expect(isPlainObject(null)).toBe(false);
  });

  it('空数组应返回 false', () => {
    expect(isPlainObject([])).toBe(false);
  });

  it('非空数组应返回 false', () => {
    expect(isPlainObject([1, 2, 3])).toBe(false);
  });

  it('字符串应返回 false', () => {
    expect(isPlainObject('hello')).toBe(false);
  });

  it('数字应返回 false', () => {
    expect(isPlainObject(0)).toBe(false);
    expect(isPlainObject(42)).toBe(false);
    expect(isPlainObject(Number.NaN)).toBe(false);
  });

  it('布尔值应返回 false', () => {
    expect(isPlainObject(true)).toBe(false);
    expect(isPlainObject(false)).toBe(false);
  });

  it('undefined 应返回 false', () => {
    expect(isPlainObject(undefined)).toBe(false);
  });

  it('函数应返回 false', () => {
    expect(isPlainObject(() => {})).toBe(false);
    expect(isPlainObject(function named() {})).toBe(false);
  });

  it('symbol / bigint 应返回 false', () => {
    expect(isPlainObject(Symbol('x'))).toBe(false);
    expect(isPlainObject(1n)).toBe(false);
  });
});

// ===========================================================================
// isPlainObject — 确定性与导出
// ===========================================================================

describe('isPlainObject — 确定性与导出', () => {
  it('应为函数类型', () => {
    expect(typeof isPlainObject).toBe('function');
  });

  it('对相同输入多次调用结果应一致', () => {
    const value = { key: 'value' };
    expect(isPlainObject(value)).toBe(isPlainObject(value));
    expect(isPlainObject(null)).toBe(isPlainObject(null));
    expect(isPlainObject([])).toBe(isPlainObject([]));
  });
});
