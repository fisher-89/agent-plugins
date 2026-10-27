/**
 * change 详情通用 Agent 面板的状态与判据单源（desktop-change-chat）：当前
 * `sessionId` 本地态（`null` = 空态 / 可发送草案态，不入禁发判据）+ 历史清单
 * 探测面（专用薄命令分页，非寻址）+ 新会话 / 历史切换 + change id 重置 +
 * 禁发与只读矩阵。
 *
 * 纪律：本 hook 不拼 `source_ref` / `cwd` 的来源词以外的字符串，不直调通用清单
 * 命令（`agent_sessions`），不按来源反查会话——清单只作探测，选中后由
 * `useAgentSession` 以 `sessionId` 直查重放。
 */
import { useCallback, useEffect, useRef, useState } from 'react';

import type { SessionPanelState } from '@/components/agent';
import { useAgentSession, type UseAgentSessionState } from '@/hooks/use-agent-session';
import { type AgentPermissionMode, type SessionSummary } from '@/types/generated/bindings';

import { ipc } from '../../../../lib/ipc';

/** 写侧来源归属受控字符串（读侧同词常量收在命令层——前后端各一份字面量） */
const CHANGE_CHAT_SOURCE = 'change';

/** 空态（草案态）文案：发送首条消息即开启新会话（不是禁用理由） */
const DRAFT_NOTE = '发送首条消息即开启新会话';

/** 历史清单分页页宽（`list_change_chat_sessions` 的 limit；「加载更多」按此累加） */
const CHANGE_CHAT_PAGE_SIZE = 20;

export interface ChangeChatParams {
  /** workspace root（清单与发起命令的 store 寻址半边） */
  root: string | null;
  /** change id（会话归属 `source_ref` 键；null = 无身份源，零 invoke） */
  changeId: string | null;
  /** 执行根来源（`detail.worktree`；null = legacy 记录，回落 workspace root） */
  worktree: string | null;
  /** 在飞 change flow run（禁发判据之一） */
  runActive: boolean;
  /** 归档 change（只读：全禁发 + 不传 cwd） */
  archived: boolean;
}

export interface ChangeChatState extends SessionPanelState {
  /** 面板本地寻址输入（null = 空态草案态） */
  sessionId: string | null;
  /** 当前会话（真相取会话镜像：首发后会话 id 由后端铸造并回写） */
  currentSessionId: string | null;
  /** 会话执行根（归档恒 null；legacy 记录为 null → 后端回落 root） */
  cwd: string | null;
  send: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  stop: () => void;
  canSend: boolean;
  disabledReason: string | null;
}

/** 历史清单探测面（取数 / 分页 / 错误；`root` 或 `changeId` 缺席即零 invoke） */
interface ChangeChatList {
  items: SessionSummary[];
  total: number;
  loading: boolean;
  error: string | null;
  reload: () => void;
  loadMore: () => void;
  clear: () => void;
}

/**
 * 清单取数面：`offset === 0` 即首屏重取（替换），否则累加追加；请求票号保证
 * 只有最新一拍落地（连点「历史」与收口重取交错时不出现旧页覆盖新页）。
 */
function useChangeChatList(params: {
  root: string | null;
  changeId: string | null;
}): ChangeChatList {
  const { root, changeId } = params;
  const [items, setItems] = useState<SessionSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ticketRef = useRef(0);

  const fetchPage = useCallback(
    (offset: number) => {
      if (root === null || changeId === null) return;
      const ticket = (ticketRef.current += 1);
      setLoading(true);
      setError(null);
      ipc
        .listChangeChatSessions(root, changeId, offset, CHANGE_CHAT_PAGE_SIZE)
        .then((page) => {
          if (ticket !== ticketRef.current) return;
          setItems((prev) => (offset === 0 ? page.sessions : [...prev, ...page.sessions]));
          setTotal(page.total);
        })
        .catch((cause: unknown) => {
          if (ticket !== ticketRef.current) return;
          setError(String(cause));
        })
        .finally(() => {
          if (ticket !== ticketRef.current) return;
          setLoading(false);
        });
    },
    [root, changeId],
  );

  const reload = useCallback(() => fetchPage(0), [fetchPage]);
  const loadMore = useCallback(() => fetchPage(items.length), [fetchPage, items.length]);
  // 复位票号即作废在途请求（change 切换后旧 change 的页不得落新 change）
  const clear = useCallback(() => {
    ticketRef.current += 1;
    setItems([]);
    setTotal(0);
    setLoading(false);
    setError(null);
  }, []);

  return { items, total, loading, error, reload, loadMore, clear };
}

/** 面板本地态（寻址输入 + 清单展开态） */
interface ChatPanelLocalState {
  sessionId: string | null;
  setSessionId: (sessionId: string | null) => void;
  listOpen: boolean;
  setListOpen: (open: boolean) => void;
}

/**
 * 面板态挂在页面身份源（URL change id）上：id 变化即复位——旧 change 的会话
 * id / 展开态 / 清单不得落到新 change，亦不发起跨 change 的取数；挂载首拍为
 * 同值复位（零 invoke）。
 */
function usePanelLocalState(changeId: string | null, clearList: () => void): ChatPanelLocalState {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [listOpen, setListOpen] = useState(false);
  useEffect(() => {
    setSessionId(null);
    setListOpen(false);
    clearList();
  }, [changeId, clearList]);
  return { sessionId, setSessionId, listOpen, setListOpen };
}

/**
 * 清单重取的两处事实驱动时机（挂载不触发、零轮询）：会话 id 由 null 变非 null
 *（首发后轮行回写）与 `running` 由 true 变 false（轮收口）——新铸会话随即出现
 * 在历史清单中可点选重开。
 */
function useListRefetch(params: {
  currentSessionId: string | null;
  running: boolean;
  reload: () => void;
}): void {
  const { currentSessionId, running, reload } = params;
  const prevSessionIdRef = useRef<string | null>(null);
  useEffect(() => {
    if (prevSessionIdRef.current === null && currentSessionId !== null) reload();
    prevSessionIdRef.current = currentSessionId;
  }, [currentSessionId, reload]);
  const wasRunningRef = useRef(running);
  useEffect(() => {
    if (wasRunningRef.current && !running) reload();
    wasRunningRef.current = running;
  }, [running, reload]);
}

/** 面板动作面（展开清单 / 加载更多 / 新会话 / 切历史 / 发送 / 停止） */
interface ChatActions {
  toggleList: () => void;
  loadMore: () => void;
  newSession: () => void;
  selectSession: (sessionId: string) => void;
  send: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  stop: () => void;
}

/**
 * 动作面装配：`toggleList` 展开即重取首屏（收起不清空——再次展开无陈旧窗口）；
 * `newSession` 清空对话本地镜像并回空态（会话行只在首条消息发出时铸造，零落
 * 库）；`send` 在整面板禁用 / 重放在途时早退，否则原样透传会话基建。
 */
function useChatActions(params: {
  session: UseAgentSessionState;
  list: ChangeChatList;
  panel: ChatPanelLocalState;
  disabledReason: string | null;
}): ChatActions {
  const { session, list, panel, disabledReason } = params;
  const { listOpen, setListOpen, setSessionId } = panel;
  const { reload, loadMore } = list;
  const { sendMessage, stop, reset, loading } = session;

  const toggleList = useCallback(() => {
    if (!listOpen) reload();
    setListOpen(!listOpen);
  }, [listOpen, reload, setListOpen]);

  const newSession = useCallback(() => {
    reset();
    setSessionId(null);
  }, [reset, setSessionId]);

  const selectSession = useCallback(
    (next: string) => {
      setSessionId(next);
      setListOpen(false);
    },
    [setSessionId, setListOpen],
  );

  const send = useCallback(
    (input: { text: string; permissionMode: AgentPermissionMode }) => {
      if (disabledReason !== null || loading) return;
      sendMessage({ prompt: input.text, permissionMode: input.permissionMode });
    },
    [disabledReason, loading, sendMessage],
  );

  return { toggleList, loadMore, newSession, selectSession, send, stop };
}

/**
 * 面板状态与判据单源（清单取数时机收敛在 [`useListRefetch`] 与 `toggleList`，
 * 挂载零清单 invoke、零轮询）。判据矩阵：归档只读 / 流程运行中 → 全禁发（历史
 * 浏览恒可用）；`sessionId === null` 不入判据也不豁免——空态即可发送草案态，
 * 首发以 `sessionId: null` 发出（后端 `New` 铸会话）。
 */
export function useChangeChat(params: ChangeChatParams): ChangeChatState {
  const { root, changeId, worktree, runActive, archived } = params;
  // 执行根派生：归档 change 的 worktree 已被 Cleanup 移除 → 恒不传（后端回落
  // root；归档行本就禁发，零 Err 路径）；legacy（worktree === null）→ null
  const cwd = archived ? null : worktree;
  const list = useChangeChatList({ root, changeId });
  const panel = usePanelLocalState(changeId, list.clear);
  // 实时面纪律：不传 `refreshKey`（run 期每条密封事件驱动详情重评估，接上去即
  // 整轮重放风暴）；会话状态重评估由会话基建自身的库态推导与订阅面承担
  const session = useAgentSession({
    root,
    sessionId: panel.sessionId,
    source: CHANGE_CHAT_SOURCE,
    sourceRef: changeId,
    cwd,
  });
  const currentSessionId = session.session?.row.id ?? null;
  useListRefetch({ currentSessionId, running: session.running, reload: list.reload });
  // 判据矩阵单源（本 hook 算一次向下传，组件不自判）
  const disabledReason = archived ? '已归档 change 只读' : runActive ? '流程运行中' : null;
  const canSend = disabledReason === null;
  const canStart = canSend && !session.running;
  const actions = useChatActions({ session, list, panel, disabledReason });
  return {
    session,
    sessionId: panel.sessionId,
    currentSessionId,
    cwd,
    sessions: list.items,
    total: list.total,
    listOpen: panel.listOpen,
    listLoading: list.loading,
    listError: list.error,
    ...actions,
    canSend,
    canStart,
    disabledReason,
    // 状态行文案（骨架只渲染）：禁用理由优先，否则空态（草案态）文案；有会话且
    // 无理由时不占位
    note: disabledReason ?? (currentSessionId === null ? DRAFT_NOTE : null),
  };
}
