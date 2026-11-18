import { ChevronLeftIcon, ChevronRightIcon, HistoryIcon, PlusCircleIcon } from 'lucide-react';
import { useCallback, useState } from 'react';

import { TextTooltip } from '@/components/standard/tooltip';
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
  ResizablePanel,
  usePanelRef,
} from '@/components/ui';
import type { UseAgentSessionState } from '@/hooks/use-agent-session';
import type { SessionSummary } from '@/types/generated/bindings';

import { AgentSessionView, type AgentSessionActions } from './agent-session-view';

/** 两处面板的公共状态子集（会话面 + 清单面 + 动作面判据 + 状态行） */
export interface SessionPanelState {
  /** 注入形态会话面（`AgentSessionView` 的 `session` 入参） */
  session: UseAgentSessionState;
  /** 历史清单本页项（`updated_at` 降序由查询面保证，分页累加） */
  sessions: SessionSummary[];
  /** 过滤集全量计数（「加载更多」可见条件 `sessions.length < total`） */
  total: number;
  listLoading: boolean;
  listError: string | null;
  onOpenHistory: () => void;
  loadMore: () => void;
  newSession: () => void;
  selectSession: (sessionId: string) => void;
  /** 「新会话」可用性单源（骨架唯一的判据消费点） */
  canStart: boolean;
  /** 状态行文案（`null` = 不占位；文案生成规则由来源侧 hook 持有） */
  note: string | null;
}

interface SessionPanelProps {
  state: SessionPanelState;
  /** 面板标题（头区标签 + 窄条竖向标题） */
  title: string;
  /** 全部查询挂钩的前缀（归属标识由消费方给出，骨架内零写死前缀） */
  testIdPrefix: string;
  /** composer 插槽（注入形态：插槽自取运行态与动作面） */
  composer: (actions: AgentSessionActions) => React.ReactNode;
}

/** 查询挂钩单点：`${前缀}-${名}`（骨架内零写死前缀） */
function testId(prefix: string, name: string): string {
  return `${prefix}-${name}`;
}

/** 头区动作面：「新会话」（禁发时一并禁）+「历史 ▾」选择（恒可用） */
function HeaderActions({
  title,
  testIdPrefix,
  state,
  canStart,
  onNewSession,
  onOpenHistory,
  onCollapse,
}: {
  title: string;
  testIdPrefix: string;
  state: SessionPanelState;
  canStart: boolean;
  onNewSession: () => void;
  onOpenHistory: () => void;
  onCollapse: () => void;
}): React.JSX.Element {
  return (
    <div
      className="flex shrink-0 items-center py-2 gap-2"
      data-testid={testId(testIdPrefix, 'actions')}
    >
      <Button
        variant="ghost"
        size="icon"
        title="收起Agent"
        aria-label="收起Agent"
        data-testid={testId(testIdPrefix, 'collapse')}
        onClick={onCollapse}
      >
        <ChevronLeftIcon />
      </Button>
      <span className="text-[13px] font-medium">{title}</span>
      <span className="flex-1" />
      <TextTooltip text="新会话">
        <Button
          variant="ghost"
          size="icon"
          aria-label="新会话"
          data-testid={testId(testIdPrefix, 'new')}
          disabled={!canStart}
          onClick={onNewSession}
        >
          <PlusCircleIcon />
        </Button>
      </TextTooltip>
      <DropdownMenu onOpenChange={(open) => open && onOpenHistory()}>
        <DropdownMenuTrigger
          render={({ onClick }) => (
            <TextTooltip text="历史会话">
              <Button
                variant="ghost"
                size="icon"
                aria-label="历史会话"
                data-testid={testId(testIdPrefix, 'history')}
                onClick={onClick}
              >
                <HistoryIcon />
              </Button>
            </TextTooltip>
          )}
        />
        <DropdownMenuContent
          data-testid={testId(testIdPrefix, 'history-list')}
          align="end"
          className="min-w-60"
        >
          <HistoryList state={state} testIdPrefix={testIdPrefix} />
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

/** 历史清单（展开态）：加载 / 错误 / 空态文案 + 分页累加（`items < total`） */
function HistoryList({
  state,
  testIdPrefix,
}: {
  state: SessionPanelState;
  testIdPrefix: string;
}): React.JSX.Element {
  if (state.listLoading && state.sessions.length === 0) {
    return <div className="text-[12px] text-muted-foreground">加载中…</div>;
  }
  if (state.listError !== null) {
    return (
      <div
        className="break-all text-[12px] text-fail"
        data-testid={testId(testIdPrefix, 'history-error')}
      >
        {`加载历史会话失败：${state.listError}`}
      </div>
    );
  }
  if (!state.listLoading && state.listError === null && state.sessions.length === 0) {
    return (
      <div
        className="text-[12px] text-muted-foreground"
        data-testid={testId(testIdPrefix, 'history-empty')}
      >
        （暂无历史会话）
      </div>
    );
  }
  return (
    <>
      <DropdownMenuRadioGroup
        className="flex flex-col gap-1"
        value={state.session.session?.row.id}
        onValueChange={state.selectSession}
      >
        {state.sessions.map((session) => (
          <DropdownMenuRadioItem
            key={session.row.id}
            value={session.row.id}
            data-testid={testId(testIdPrefix, 'history-item')}
            closeOnClick
          >
            <div>
              <div className="block font-mono">{session.row.id}</div>
              <div className="text-muted-foreground">
                {new Date(session.row.updatedAt).toLocaleString()} · {session.stats.turnCount} 轮
              </div>
            </div>
          </DropdownMenuRadioItem>
        ))}
      </DropdownMenuRadioGroup>
      {state.sessions.length < state.total && (
        <button
          type="button"
          className="mt-1 w-full cursor-pointer rounded-md border border-border bg-transparent px-2 py-1 text-[12px] hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
          data-testid={testId(testIdPrefix, 'history-more')}
          disabled={state.listLoading}
          onClick={state.loadMore}
        >
          加载更多
        </button>
      )}
    </>
  );
}

/** 折叠窄条：竖向标题 + 展开入口（收起态仍留可见入口） */
function CollapsedRail({
  title,
  testIdPrefix,
  onExpand,
}: {
  title: string;
  testIdPrefix: string;
  onExpand: () => void;
}): React.JSX.Element {
  return (
    <div
      className="flex h-full flex-col items-center gap-2 py-2"
      data-testid={testId(testIdPrefix, 'rail')}
    >
      <Button
        variant="ghost"
        size="icon"
        data-testid={testId(testIdPrefix, 'expand')}
        title={`展开${title}`}
        onClick={onExpand}
      >
        <ChevronRightIcon />
      </Button>
      <span className="text-[12px] text-muted-foreground [writing-mode:vertical-rl]">{title}</span>
    </div>
  );
}

const MIN_SIZE = 500;

/**
 * 会话面板骨架
 */
export function SessionPanel(props: SessionPanelProps): React.JSX.Element {
  const { state, title, testIdPrefix, composer } = props;

  const panelRef = usePanelRef();
  const [isCollapsed, setIsCollapsed] = useState(true);

  const expand = useCallback(() => {
    panelRef.current?.expand();
    setIsCollapsed(false);
  }, [panelRef]);

  const collapse = useCallback(() => {
    panelRef.current?.collapse();
    setIsCollapsed(true);
  }, [panelRef]);

  return (
    <ResizablePanel
      defaultSize="32px"
      maxSize={`${MIN_SIZE * 2}px`}
      minSize={`${MIN_SIZE}px`}
      collapsible={true}
      collapsedSize="32px"
      collapsedThreshold={`${MIN_SIZE / 2}px`}
      onResize={({ inPixels }) => setIsCollapsed(inPixels < MIN_SIZE)}
      panelRef={panelRef}
      data-testid={testId(testIdPrefix, 'panel')}
    >
      {isCollapsed ? (
        <CollapsedRail title={title} testIdPrefix={testIdPrefix} onExpand={expand} />
      ) : (
        <ExpandedBody
          state={state}
          title={title}
          testIdPrefix={testIdPrefix}
          composer={composer}
          onCollapse={collapse}
        />
      )}
    </ResizablePanel>
  );
}

/** 展开态面板体：头区动作 → 状态行 → 历史清单 → 会话体 + composer 插槽 */
function ExpandedBody({
  state,
  title,
  testIdPrefix,
  composer,
  onCollapse,
}: {
  state: SessionPanelState;
  title: string;
  testIdPrefix: string;
  composer: (actions: AgentSessionActions) => React.ReactNode;
  onCollapse: () => void;
}): React.JSX.Element {
  return (
    <div className="flex h-full min-h-0 flex-col gap-2 pr-1.5 min-w-100">
      <HeaderActions
        title={title}
        testIdPrefix={testIdPrefix}
        state={state}
        canStart={state.canStart}
        onNewSession={state.newSession}
        onOpenHistory={state.onOpenHistory}
        onCollapse={onCollapse}
      />
      {state.note !== null && (
        <div
          className="shrink-0 text-[12px] text-muted-foreground"
          data-testid={testId(testIdPrefix, 'note')}
        >
          {state.note}
        </div>
      )}
      <AgentSessionView
        session={state.session}
        composer={composer}
        // 骨架合成动作面：archi / change-chat 两处 hook 与面板零改动即得活卡
        askAnswer={{ onAnswer: state.session.answer }}
        data-testid={testId(testIdPrefix, 'session')}
      />
    </div>
  );
}
