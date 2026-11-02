import { useEffect, useRef, useState, type RefObject } from 'react';

import { useFileWatchSignal } from '../../../hooks/use-file-watch-signal';
import { ipc } from '../../../lib/ipc';
import type { ChangeStepStatus } from '../../../types/dto';

export interface UseExecOutputResult {
  /** 执行命令文本（sidecar `<exec_id>.cmd`）；null = 老记录无 sidecar / 尚未取到 */
  command: string | null;
  /** 脚本真实输出（UTF-8；**原始**文本——`\r` 归一由面板渲染前做）；null = 未取数 */
  text: string | null;
  /** 取数失败（invoke 拒绝）串；miss（返回 null）非错误 */
  error: string | null;
}

/** 错误态文案口径（沿既有「invoke reject → error 态」单点）。 */
function toMessage(cause: unknown): string {
  return typeof cause === 'string' ? cause : String(cause);
}

/**
 * 命令头行：按寻址三元组读一次（依赖面**不含** `status`——命令文本不随步状态
 * 变化）。读取失败静默降级为 `null`（面板呈「命令未记录」），MUST NOT 阻断内容
 * 面、MUST NOT 写 `error`（命令面缺失不是内容面故障）。
 */
function useCommandText(
  root: string | null,
  changeName: string | null,
  execId: string | null,
): string | null {
  const [command, setCommand] = useState<string | null>(null);
  useEffect(() => {
    if (root === null || changeName === null || execId === null) {
      setCommand(null);
      return;
    }
    let disposed = false;
    ipc
      .readExecCommand(root, changeName, execId)
      .then((value) => {
        if (!disposed) setCommand(value ?? null);
      })
      .catch(() => {
        if (!disposed) setCommand(null);
      });
    return () => {
      disposed = true;
    };
  }, [root, changeName, execId]);
  return command;
}

/** 增量读句柄：`read` 由装载首拍与 watch 信号共用，`dispose` 作废在飞响应。 */
interface DeltaReader {
  read: () => void;
  dispose: () => void;
}

/** 三枚 ref 的复位口径（臂起点 / 空参臂 / cleanup 同一处）：游标归 0、两枚标记清零。 */
function resetDeltaRefs(
  offsetRef: RefObject<number>,
  pendingRef: RefObject<boolean>,
  queuedRef: RefObject<boolean>,
): void {
  offsetRef.current = 0;
  pendingRef.current = false;
  queuedRef.current = false;
}

/**
 * 同臂单飞增量读（D10-a）。三条规则：
 * ① **入口短路**：已作废直接返回（**不动任何标记**）；在飞时**丢弃本次调用、只记
 * 一次排队**——不叠读、不动游标。无守卫时同臂两次读共享同一 `offsetRef`（后端
 * `read_exec_output_delta_in` 是整文件读、非瞬时，`SIGNAL_DEBOUNCE_MS` 只合并
 * **信号**、不覆盖**在飞时长**）⇒ 同段文本被追加两遍（「连续增量拼接 == 全量」
 * 不成立），且两次 `nextOffset` 同值（offset 推进无从观测）。
 * ② **结算顺序**：`then` / `catch` 内**先**清 `pendingRef`、**再**推 `offsetRef`
 * 与追加文本——标记先于游标落定，尾读不会用旧游标抢跑。
 * ③ **尾读折叠（有界）**：结算末尾发现排队则补**一拍**。增量读是「全量快照 +
 * 游标」语义，一拍即吃掉其间全部字节 ⇒ 队列长度恒 ≤ 1，不随信号频率累积。
 */
function createDeltaReader(params: {
  root: string;
  changeName: string;
  execId: string;
  offsetRef: RefObject<number>;
  pendingRef: RefObject<boolean>;
  queuedRef: RefObject<boolean>;
  onText: (chunk: string) => void;
  onError: (message: string) => void;
}): DeltaReader {
  const { root, changeName, execId, offsetRef, pendingRef, queuedRef, onText, onError } = params;
  let disposed = false;
  /** 尾读：至多一拍（`read` 再入时 pending 已清 ⇒ 必然发出，形成有界链） */
  function settle(): void {
    if (!queuedRef.current) return;
    queuedRef.current = false;
    read();
  }
  function read(): void {
    if (disposed) return;
    if (pendingRef.current) {
      queuedRef.current = true;
      return;
    }
    pendingRef.current = true;
    void ipc
      .readExecOutputDelta(root, changeName, execId, offsetRef.current)
      .then((chunk) => {
        if (disposed) return;
        pendingRef.current = false;
        if (chunk !== null) {
          offsetRef.current = chunk.nextOffset;
          if (chunk.text !== '') onText(chunk.text);
        }
        settle();
      })
      .catch((cause: unknown) => {
        if (disposed) return;
        pendingRef.current = false;
        onError(toMessage(cause));
        settle();
      });
  }
  return {
    read,
    dispose: () => {
      disposed = true;
    },
  };
}

/**
 * 内容面：每次臂变化（含 `status` 迁移）都从 offset 0 起读一次全量——装载首拍
 * 覆盖「订阅前已产生的字节」，转终态则吸收收口后的权威内容重排（`nextOffset`
 * 唯一产出方在后端，前端零字节计数）。返回的引用供 watch 信号回调再次取增量
 * （同臂单飞由 `createDeltaReader` 承担）。
 */
function useOutputText(
  root: string | null,
  changeName: string | null,
  execId: string | null,
  status: ChangeStepStatus | null,
): {
  text: string | null;
  error: string | null;
  readRef: RefObject<(() => void) | null>;
} {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** 游标（初值 0）：唯一来源是后端 `nextOffset`；每条臂起点（含 `status` 迁移）归 0 */
  const offsetRef = useRef(0);
  /** 在飞标记（初值 false）：置真即一次增量读在飞；结算（`then` / `catch`）与臂起点 /
   * cleanup 一律归 false —— 在飞期间新到的请求只置 `queuedRef`，不叠读 */
  const pendingRef = useRef(false);
  /** 排队标记（初值 false）：在飞期间收到信号即置真，结算末尾读出并清零后补**一拍**
   * 尾读（长度恒 ≤ 1，不随信号数增长）；臂起点 / cleanup 归 false */
  const queuedRef = useRef(false);
  const readRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    if (root === null || changeName === null || execId === null) {
      setText(null);
      setError(null);
      readRef.current = null;
      resetDeltaRefs(offsetRef, pendingRef, queuedRef);
      return;
    }
    resetDeltaRefs(offsetRef, pendingRef, queuedRef);
    setError(null);
    setText(null);
    const reader = createDeltaReader({
      root,
      changeName,
      execId,
      offsetRef,
      pendingRef,
      queuedRef,
      onText: (chunk) => setText((prev) => (prev ?? '') + chunk),
      onError: setError,
    });
    readRef.current = reader.read;
    reader.read();
    return () => {
      reader.dispose();
      resetDeltaRefs(offsetRef, pendingRef, queuedRef);
      readRef.current = null;
    };
  }, [root, changeName, execId, status]);

  return { text, error, readRef };
}

/**
 * exec 输出记录取数（stdout 面板数据源）。四参：
 * - `root` / `changeName`（磁盘面按 change **name** 定位，与 `record_exec_output`
 *   同口径；name 取 `detail.name`，非 id）/ `execId`（节点 `outputRef`）；
 * - `status`（节点 `status`，由 `DetailDrawer` 下传）——运行中挂 watch 增量跟随，
 *   转终态退订并整读一次。
 *
 * **一条状态规则解决两件事**：内容面 effect 依赖含 `status`，故 `status` 转非
 * running 即新 effect 实例 ⇒ `offsetRef` 归零 ⇒ 首拍 `fromOffset = 0` 整读一次。
 * 该规则同时吸收「收口时权威内容可能重排」（test-execution 的 stdout 前缀 +
 * stderr 后缀覆盖 live 交错流）与「文件被覆盖后旧 offset 越界」（后端夹 0，前端
 * 零特判）。
 *
 * `nextOffset` 的唯一产出方在后端：前端 MUST NOT 按字符串长度累加游标（否则
 * 「JS 字符数 vs UTF-8 字节数」的口径分裂立刻显形）。
 *
 * **同臂单飞**：一次增量读在飞时新到的请求被丢弃并置 `queuedRef`（不叠读、不动
 * 游标），结算时先清 `pendingRef` 再推 `offsetRef`，末尾至多补**一拍**尾读。后端
 * 是「无状态快照 + 游标」语义、防抖只合并信号 ⇒ 无守卫即静默破约：同段文本双写
 * （「连续增量拼接 == 全量」不成立）、`nextOffset` 两拍同值（offset 推进不可观测）。
 *
 * 取数全部经 `ipc` 代理面（MUST NOT 绕回裸 `commands.*`）；watch 订阅经共享件
 * `useFileWatchSignal`（零自建 `Channel` / 零直呼 watch 命令 / 零自建防抖）。
 */
export function useExecOutput(params: {
  root: string | null;
  changeName: string | null;
  execId: string | null;
  status: ChangeStepStatus | null;
}): UseExecOutputResult {
  const { root, changeName, execId, status } = params;
  const command = useCommandText(root, changeName, execId);
  const { text, error, readRef } = useOutputText(root, changeName, execId, status);

  // 运行中增量跟随：`armKey` 非运行态为 null ⇒ 零 Channel、零 watch_subscribe
  //（收口后文件已冻结，订阅无意义）。防抖窗口沿共享件的 SIGNAL_DEBOUNCE_MS。
  useFileWatchSignal({
    armKey:
      root !== null && execId !== null && status === 'running' ? `${root}\u0000${execId}` : null,
    resolvePath: () =>
      root === null || changeName === null || execId === null
        ? Promise.resolve(null)
        : ipc.execOutputPath(root, changeName, execId),
    onSignal: () => readRef.current?.(),
  });

  return { command, text, error };
}
