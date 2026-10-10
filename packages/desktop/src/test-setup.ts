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
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  Object.assign(globalThis, { ResizeObserver: ResizeObserverStub });
}
