import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { useOpen } from './use-open';

// 零进程边界：React hooks 真实承载（renderHook + act + rerender），不 mock 被测实现；
// onOuterOpenChange 以稳定 vi.fn() 单实例注入——handleOpenChange 为 useCallback(..., [])，
// 闭包捕获首渲染回调（R1 现状锁定），故每档 rerender 沿用同一实例、MUST NOT 中途替换。

type HookProps = {
  outerOpen?: boolean;
  onOuterOpenChange?: (value: boolean) => void;
};

/** 挂载 useOpen（入参经 props 透传，供 rerender 逐档注入）。 */
function mountOpen(initialProps: HookProps) {
  return renderHook((props: HookProps) => useOpen(props.outerOpen, props.onOuterOpenChange), {
    initialProps,
  });
}

describe('useOpen：初始态（非受控缺省 / 显式 true / 两可选入参同缺）', () => {
  it('非受控缺省入参（outerOpen 省略）：初始返回 open=false，变更函数为二元组第二位', () => {
    const { result } = mountOpen({});

    expect(result.current).toHaveLength(2);
    expect(result.current[0]).toBe(false);
    expect(typeof result.current[1]).toBe('function');
  });

  it('非受控初始 outerOpen=true：初始返回 open=true', () => {
    const { result } = mountOpen({ outerOpen: true });

    expect(result.current[0]).toBe(true);
  });

  it('两可选入参同缺（全缺省调用）：初始 false、setOpen(true/false) 不抛错且状态照常切换', () => {
    const { result } = mountOpen({});

    expect(result.current[0]).toBe(false);
    act(() => {
      result.current[1](true);
    });
    expect(result.current[0]).toBe(true);
    act(() => {
      result.current[1](false);
    });
    expect(result.current[0]).toBe(false);
  });
});

describe('useOpen：变更函数与回调（布尔全档与回调缺省安全）', () => {
  it('setOpen(true) → open 同步 true 且回调恰收 true；setOpen(false) → 回落 false 回调收 false', () => {
    const onOuterOpenChange = vi.fn<(value: boolean) => void>();
    const { result } = mountOpen({ onOuterOpenChange });

    act(() => {
      result.current[1](true);
    });
    expect(result.current[0]).toBe(true);
    expect(onOuterOpenChange).toHaveBeenCalledTimes(1);
    expect(onOuterOpenChange).toHaveBeenCalledWith(true);

    act(() => {
      result.current[1](false);
    });
    expect(result.current[0]).toBe(false);
    expect(onOuterOpenChange).toHaveBeenCalledTimes(2);
    expect(onOuterOpenChange).toHaveBeenNthCalledWith(2, false);
  });

  it('onOuterOpenChange 缺席（undefined，Optional None 档）：setOpen(true/false) 不抛错且状态照常切换', () => {
    const { result } = mountOpen({ outerOpen: false });

    expect(() => {
      act(() => {
        result.current[1](true);
      });
    }).not.toThrow();
    expect(result.current[0]).toBe(true);
    expect(() => {
      act(() => {
        result.current[1](false);
      });
    }).not.toThrow();
    expect(result.current[0]).toBe(false);
  });
});

describe('useOpen：受控同步（outerOpen 注入驱动内部态）', () => {
  it('受控同步：rerender 注入 outerOpen false → true，返回 open 随 prop 切换（prop 同步不发回调）', () => {
    const onOuterOpenChange = vi.fn<(value: boolean) => void>();
    const { result, rerender } = mountOpen({ outerOpen: false, onOuterOpenChange });
    expect(result.current[0]).toBe(false);

    rerender({ outerOpen: true, onOuterOpenChange });

    expect(result.current[0]).toBe(true);
    // 回调只由变更函数触发：纯 prop 同步不重发
    expect(onOuterOpenChange).not.toHaveBeenCalled();
  });

  it('outerOpen 注入 undefined（rerender 注入，Optional None 档）：不干预内部态（保持 setOpen 后的值）', () => {
    const onOuterOpenChange = vi.fn<(value: boolean) => void>();
    const { result, rerender } = mountOpen({ outerOpen: true, onOuterOpenChange });

    act(() => {
      result.current[1](false);
    });
    expect(result.current[0]).toBe(false);

    rerender({ outerOpen: undefined, onOuterOpenChange });

    expect(result.current[0]).toBe(false);
    expect(onOuterOpenChange).toHaveBeenCalledTimes(1);
  });
});

describe('useOpen：注入边界（同值不重设 / 异值覆盖 / 三态矩阵）', () => {
  it('同值注入：内部态已 true 时注入 outerOpen=true 不重设（状态保持 true、回调不重发）', () => {
    const onOuterOpenChange = vi.fn<(value: boolean) => void>();
    const { result, rerender } = mountOpen({ outerOpen: false, onOuterOpenChange });

    act(() => {
      result.current[1](true);
    });
    expect(result.current[0]).toBe(true);

    rerender({ outerOpen: true, onOuterOpenChange });

    expect(result.current[0]).toBe(true);
    expect(onOuterOpenChange).toHaveBeenCalledTimes(1);
  });

  it('异值注入：内部态 true 而注入 outerOpen=false 时被同步覆盖为 false', () => {
    const onOuterOpenChange = vi.fn<(value: boolean) => void>();
    // outerOpen 起步 undefined（非受控）：变更函数先落内部态
    const { result, rerender } = mountOpen({ onOuterOpenChange });

    act(() => {
      result.current[1](true);
    });
    expect(result.current[0]).toBe(true);

    rerender({ outerOpen: false, onOuterOpenChange });

    expect(result.current[0]).toBe(false);
  });

  it('三态矩阵·逐档初始：true / false / undefined 初始各归其位（undefined 起步为 false）', () => {
    const fromTrue = mountOpen({ outerOpen: true });
    expect(fromTrue.result.current[0]).toBe(true);
    fromTrue.unmount();

    const fromFalse = mountOpen({ outerOpen: false });
    expect(fromFalse.result.current[0]).toBe(false);
    fromFalse.unmount();

    const fromUndefined = mountOpen({});
    expect(fromUndefined.result.current[0]).toBe(false);
  });

  it('三态矩阵·逐档 rerender 注入：true → false → undefined → true 逐档同步，undefined 档不干预', () => {
    const { result, rerender } = mountOpen({ outerOpen: true });
    expect(result.current[0]).toBe(true);

    rerender({ outerOpen: false });
    expect(result.current[0]).toBe(false);

    // undefined 注入不干预：保持上一档 false
    rerender({ outerOpen: undefined });
    expect(result.current[0]).toBe(false);

    rerender({ outerOpen: true });
    expect(result.current[0]).toBe(true);
  });
});
