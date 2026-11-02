/**
 * stdout 面板的两条**渲染前**纯判据：`\r` 归一与粘底判定。
 *
 * ① 两者都是渲染前判据，MUST NOT 进取数面——`useExecOutput` 返回**原始**累积
 * 文本（故「终态整读与整读一致」的断言不被归一改变），面板渲染时才收敛 `\r`；
 * 粘底判据只吃容器三指标，与数据面无关。
 *
 * ② 落本文件而非组件内嵌的原因：knip 的 `project` 排除测试文件 ⇒ 组件内为测试
 * 开 `export` 会被判 `Unused exports`（AGENT.md 规则 5 禁 test-only 导出）。本仓
 * 既有同型解是纯判据落模块（`flow/metrics.ts` / `flow/run-state.ts` /
 * `flow/layout.ts`），由组件 import 消费。
 *
 * ③ 判据边界：`\r` 归一逐行只保留最后一个 `\r` 之后的内容（cargo 进度行不在
 * `<pre>` 中堆成残行）；粘底容差 1 px（`scrollHeight - scrollTop - clientHeight
 * <= 1`）——上滑即 `false`，回底恢复 `true`。
 */

/**
 * `\r` 归一：逐行只保留最后一个 `\r` 之后的内容（同行刷新序列收敛为末态）。
 * 纯函数、零副作用；无 `\r` 的行原样返回。
 */
export function normalizeCarriageReturns(text: string): string {
  return text
    .split('\n')
    .map((line) => {
      const index = line.lastIndexOf('\r');
      return index < 0 ? line : line.slice(index + 1);
    })
    .join('\n');
}

/**
 * 粘底判据：滚到底部（容差 1 px）为 `true`——新输出到达时应自动跟随；用户上滑
 * 即 `false`（停止跟随），回底恢复。纯函数、零 DOM 依赖（组件侧传 `el` 的三
 * 指标）。
 */
export function isStuckToBottom(metrics: {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}): boolean {
  return metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight <= 1;
}
