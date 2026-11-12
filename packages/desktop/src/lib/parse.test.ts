import { describe, expect, it } from 'vite-plus/test';

import { asRecord } from './parse';

describe('asRecord对象化判断', () => {
  it.each([{}, { k: 'v' }, { k: { ks: 1 } }, { 1: 'v' }])('键值对象类型返回等价对象', (input) => {
    expect(asRecord(input)).toEqual(input);
  });

  it.each([new Map([['k1', 'v1']])])('Map类型返回同结构键值对象', (input) => {
    expect(asRecord(input)).toEqual(Object.fromEntries(input.entries()));
  });

  it.each([5, 'str', ['v'], null])('非键值对象类型返回空对象', (input) => {
    expect(asRecord(input)).toEqual({});
  });

  it('过滤对象中Symbol类型的键', () => {
    expect(asRecord({ [Symbol('k1')]: 'v1', k2: 'v2' })).toEqual({ k2: 'v2' });
  });
});
