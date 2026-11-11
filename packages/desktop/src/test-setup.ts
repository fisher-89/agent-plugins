import { configure } from '@testing-library/react';

// jsdom 缺 Web Animations API，@base-ui 有两个消费面需要分别兜底：
// 1. ScrollArea 视口的滚动收尾探测无条件调用 viewport.getAnimations({subtree})
//    ——补空实现（无动画在进行）满足探测语义，消除未捕获异常；
// 2. useAnimationsFinished（退出卸载时序）在 API 在场时改走微任务异步收尾，
//    同步断言会看到「旧面板未卸载」的中间态——置官方开关
//    BASE_UI_ANIMATIONS_DISABLED 让其回落同步路径（与 API 缺席时同构）。
if (typeof Element.prototype.getAnimations !== 'function') {
  Element.prototype.getAnimations = () => [];
}
Object.assign(globalThis, { BASE_UI_ANIMATIONS_DISABLED: true });

// jsdom 缺 ResizeObserver：react-resizable-panels（explore 详情页双栏）在布局
// effect 内 new ResizeObserver 观测面板尺寸——补空实现（零回调、零上报）满足
// 挂载 / 卸载语义。
if (typeof globalThis.ResizeObserver !== 'function') {
  class ResizeObserverStub {
    public observe(): void {}
    public unobserve(): void {}
    public disconnect(): void {}
  }
  Object.assign(globalThis, { ResizeObserver: ResizeObserverStub });
}

// jsdom 缺 PointerEvent：@base-ui 的 Checkbox / Button 在 click 激活时用
// ownerWindow(target).PointerEvent 构造带修饰键的 click 事件（原生 click()
// 恒报「无修饰键」）——补最小实现（继承 MouseEvent，零附加字段）满足构造语义，
// 否则勾选点击抛 "PointerEvent is not a constructor"（未捕获异常）。
if (typeof globalThis.PointerEvent !== 'function') {
  class PointerEventStub extends MouseEvent {}
  Object.assign(globalThis, { PointerEvent: PointerEventStub });
}

// 虚拟滚动视口尺寸桩：jsdom 无布局（`clientHeight` / `scrollHeight` 恒 0），
// `@tanstack/virtual-core` 的视口测量经 `getRect` 同步读这两个值（挂载即回调，
// 不依赖 ResizeObserver 的后续上报）⇒ 不打桩时 outerSize = 0、可见 range 恒空，
// 所有气泡断言会变成 0 条。
// 1. 只补两个 getter：项高度不在这里——虚拟项的零测量护栏（实测 ≤ 0 回落
//    estimateSize）已兜住，故不桩 `getBoundingClientRect`、不桩 `clientWidth`；
//    `scrollHeight` 只决定 `getDistanceFromEnd` 的分母。
// 2. 原型级 getter：仓库既有实例级 `Object.defineProperty` 打桩优先于原型，互不
//    干扰；`clientHeight = scrollHeight = 600` ⇒ maxScroll = 0 ⇒ 全局呈现「内容
//    不足一屏且贴底」，需要非贴底形态的用例在自身内局部抬高 `scrollHeight`。
// 3. 作用面按 `data-slot="message-scroller-viewport"` 收窄，不影响其余套件。
const VIEWPORT_SLOT = 'message-scroller-viewport';
const VIEWPORT_STUB_SIZE = 600;

function stubViewportSize(property: 'clientHeight' | 'scrollHeight'): void {
  const original = Object.getOwnPropertyDescriptor(Element.prototype, property);
  if (original?.get === undefined) return;
  Object.defineProperty(Element.prototype, property, {
    ...original,
    get(this: Element): number {
      if (this.getAttribute('data-slot') === VIEWPORT_SLOT) return VIEWPORT_STUB_SIZE;
      return original.get?.call(this) ?? 0;
    },
  });
}
stubViewportSize('clientHeight');
stubViewportSize('scrollHeight');

// 测试门禁高并行运行（覆盖度量 + rust llvm-cov 编译抢 CPU）时 jsdom 定时器 /
// rAF 可被饿死：RTL 异步等待（waitFor / findBy）默认 1s 预算会被负载抖动打穿
// （抽屉弹层等异步呈现断言偶发超时）。放宽到 5s——等待语义不变，仅失败反馈变慢。
configure({ asyncUtilTimeout: 5000 });
