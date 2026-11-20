/**
 * 配置页常驻 Agent 面板的状态与判据单源（`desktop-config-agent`）：会话归属固定
 * `source = "config"` / `sourceRef = null` / `cwd = null`（后端回落 workspace root
 * = 主仓根）+ 历史清单探测面（专用薄命令分页，非寻址）+ 新会话 / 历史切换 +
 * root 复位 + 运行中禁发判据 + 状态行两态。
 *
 * 纪律：本 hook 不直调通用会话清单命令、不按来源反查会话——清单只作探测，选中
 * 后由 `useAgentSession` 以 `sessionId` 直查重放；配置页无归档态与 flow run 判据，
 * 禁发矩阵只有本会话 `running` 一条。**写回动作不经本 hook**：它由面板经
 * `ConfigApplyScope` 交给提案卡（页面取数路径只有 `useWorkspaceConfig`）。
 */
import { useCallback, useEffect, useRef, useState } from 'react';

import type { SessionPanelState } from '@/components/agent';
import { useAgentSession, type UseAgentSessionState } from '@/hooks/use-agent-session';
import { ipc } from '@/lib/ipc';
import type { AgentPermissionMode, SessionSummary } from '@/types/generated/bindings';

/** 写侧来源归属受控字符串（与 `compose::CONFIG_SOURCE`、命令层
 * `CONFIG_CHAT_SOURCE` 三处同字面量，改动需同步） */
const CONFIG_CHAT_SOURCE = 'config';

/** 历史清单分页页宽（`list_config_chat_sessions` 的 limit；「加载更多」按此累加） */
const CONFIG_CHAT_PAGE_SIZE = 20;

/** 空态（草案态）文案：发送首条消息即开启新会话（不是禁用理由） */
const DRAFT_NOTE = '发送首条消息即开启新会话';

/** 已开会话且无禁用理由时的提醒：写回只在提案卡上点「应用」生效 */
const WRITEBACK_NOTE = '配置写回只在提案卡上点「应用」生效';

export interface ConfigChatParams {
  /** workspace root（清单与发起的 store 寻址半边；null = 零 invoke） */
  root: string | null;
}

export interface ConfigChatState extends SessionPanelState {
  /** 面板本地寻址输入（null = 空态草案态） */
  sessionId: string | null;
  /** 当前会话（真相取会话镜像：首发后会话 id 由后端铸造并回写） */
  currentSessionId: string | null;
  send: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  stop: () => void;
  canSend: boolean;
  disabledReason: string | null;
}

/** 历史清单探测面（取数 / 分页 / 错误；`root` 缺席即零 invoke） */
interface ConfigChatList {
  items: SessionSummary[];
  total: number;
  loading: boolean;
  error: string | null;
  reload: () => void;
  loadMore: () => void;
  clear: () => void;
}

/**
 * 清单取数面：`offset === 0` 即首屏重取（替换），否则累加追加；请求票号保证只有
 * 最新一拍落地（连点「历史」与收口重取交错时不出现旧页覆盖新页）。
 */
function useConfigChatList({ root }: { root: string | null }): ConfigChatList {
  const [items, setItems] = useState<SessionSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ticketRef = useRef(0);

  const fetchPage = useCallback(
    (offset: number) => {
      if (root === null) return;
      const ticket = (ticketRef.current += 1);
      setLoading(true);
      setError(null);
      ipc
        .listConfigChatSessions(root, offset, CONFIG_CHAT_PAGE_SIZE)
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
    [root],
  );

  const reload = useCallback(() => fetchPage(0), [fetchPage]);
  const loadMore = useCallback(() => fetchPage(items.length), [fetchPage, items.length]);
  // 复位票号即作废在途请求（切根后旧根的页不得落新根）
  const clear = useCallback(() => {
    ticketRef.current += 1;
    setItems([]);
    setTotal(0);
    setLoading(false);
    setError(null);
  }, []);

  return { items, total, loading, error, reload, loadMore, clear };
}

/** 面板本地态（寻址输入） */
interface ChatPanelLocalState {
  sessionId: string | null;
  setSessionId: (sessionId: string | null) => void;
}

/**
 * 面板态挂在归属身份源（当前 workspace root）上：root 变化即复位——旧根的会话
 * id / 清单不得落到新根，亦不发起跨根取数；挂载首拍为同值复位（零 invoke）。
 */
function usePanelLocalState(root: string | null, clearList: () => void): ChatPanelLocalState {
  const [sessionId, setSessionId] = useState<string | null>(null);
  useEffect(() => {
    setSessionId(null);
    clearList();
  }, [root, clearList]);
  return { sessionId, setSessionId };
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
interface ChatActions extends Pick<
  SessionPanelState,
  'onOpenHistory' | 'loadMore' | 'newSession' | 'selectSession'
> {
  send: (input: { text: string; permissionMode: AgentPermissionMode }) => void;
  stop: () => void;
}

/**
 * 动作面装配：`newSession` 清空对话本地镜像并回空态（会话行只在首条消息发出时铸造，
 * 零落库）；`send` 在运行中（唯一禁用判据）或重放在途时早退，否则原样透传会话基建。
 */
function useChatActions(params: {
  session: UseAgentSessionState;
  list: ConfigChatList;
  panel: ChatPanelLocalState;
  disabledReason: string | null;
}): ChatActions {
  const { session, list, panel, disabledReason } = params;
  const { setSessionId } = panel;
  const { reload, loadMore } = list;
  const { sendMessage, stop, reset, loading } = session;

  const onOpenHistory = useCallback(() => {
    reload();
  }, [reload]);

  const newSession = useCallback(() => {
    reset();
    setSessionId(null);
  }, [reset, setSessionId]);

  const selectSession = useCallback(
    (next: string) => {
      setSessionId(next);
    },
    [setSessionId],
  );

  const send = useCallback(
    (input: { text: string; permissionMode: AgentPermissionMode }) => {
      if (disabledReason !== null || loading) return;
      sendMessage({ prompt: input.text, permissionMode: input.permissionMode });
    },
    [disabledReason, loading, sendMessage],
  );

  return { onOpenHistory, loadMore, newSession, selectSession, send, stop };
}

/**
 * 面板状态与判据单源（挂载零清单 invoke、零轮询）。判据矩阵：本会话 `running` →
 * 禁发（连「新会话」一并禁，历史浏览恒可用）；`sessionId === null` 不入判据也不
 * 豁免——空态即可发送，首发以 `sessionId: null` 发出（后端 `New` 铸会话）。
 */
export function useConfigChat(params: ConfigChatParams): ConfigChatState {
  const { root } = params;
  const list = useConfigChatList({ root });
  const panel = usePanelLocalState(root, list.clear);
  // `cwd = null` 即后端回落 workspace root（主仓根）；`sourceRef = null` 表示
  // 一个 workspace 一份 config 历史，不挂 change / explore / archi 身份
  const session = useAgentSession({
    root,
    sessionId: panel.sessionId,
    source: CONFIG_CHAT_SOURCE,
    sourceRef: null,
    cwd: null,
  });
  const currentSessionId = session.session?.row.id ?? null;
  useListRefetch({ currentSessionId, running: session.running, reload: list.reload });
  // 判据矩阵单源（本 hook 算一次向下传，骨架与 composer 不自判）
  const disabledReason = session.running ? '本会话运行中，请等待收口' : null;
  const canSend = disabledReason === null;
  const canStart = disabledReason === null;
  const actions = useChatActions({ session, list, panel, disabledReason });
  return {
    session,
    sessionId: panel.sessionId,
    currentSessionId,
    sessions: list.items,
    total: list.total,
    listLoading: list.loading,
    listError: list.error,
    ...actions,
    canSend,
    canStart,
    disabledReason,
    // 状态行文案（骨架只渲染）：禁用理由优先，否则空态草案文案 / 已开会话的写回提醒
    note: disabledReason ?? (currentSessionId === null ? DRAFT_NOTE : WRITEBACK_NOTE),
  };
}
