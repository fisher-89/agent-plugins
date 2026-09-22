import { describe, expect, it } from 'vite-plus/test';

import { cn } from './utils';

// cn 是 clsx + tailwind-merge 的薄组装层：只测组装契约（条件拼接、falsy 剔除、冲突取后者），
// 不逐项验证 tailwind-merge 自带的匹配/合并语义（既有测试纪律）。
describe('cn：类名组合与冲突消解的唯一入口', () => {
  it('条件表达式为真时保留、为假时剔除，输出仅含真值类名', () => {
    const condTruthy: boolean = true;
    const condFalsy: boolean = false;
    expect(cn('a', condTruthy && 'b', 'c')).toBe('a b c');
    expect(cn('a', condFalsy && 'b', 'c')).toBe('a c');
  });

  it('无参调用与全 falsy 输入返回空字符串', () => {
    expect(cn()).toBe('');
    expect(cn(false, undefined, '')).toBe('');
  });

  it('undefined / null / false 混入被忽略，输出不含 undefined / null 字样', () => {
    const merged = cn('a', undefined, null, false);
    expect(merged).toBe('a');
    expect(merged).not.toContain('undefined');
    expect(merged).not.toContain('null');
  });

  it('冲突 utilities 后者胜（组装契约冒烟）', () => {
    expect(cn('px-2', 'px-4')).toBe('px-4');
  });
});
