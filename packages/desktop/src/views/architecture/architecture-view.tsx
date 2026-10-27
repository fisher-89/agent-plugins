/**
 * 架构页（挂载点 A）：观察根 = 壳态当前根（URL 不携根段），`mode = "full"` 全量
 * 只读——不呈现任何可提交的改档入口。
 *
 * 布局：整宽页面头（标题 + 面板开关 + 刷新）留在横向分栏之上；分栏左列为通用
 * Agent 面板（`collapsible`、默认收起、收起态留窄条），右列为画布。折叠态真相
 * 唯一来源 = 页面开关 state（MUST NOT 取分栏 resize 回调的像素派生——架构页的
 * 折叠只由显式开关驱动，该差异对 change 详情左栏属刻意）。
 *
 * 取数**显式**：刷新按钮以 `key` 重挂画布（挂载即取数），零 watch 零轮询；磁盘上
 * 改动模型后须用户点刷新才可见（会话写盘同理，故面板状态行提示刷新）。
 */
import { useCallback, useLayoutEffect, useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  usePanelRef,
} from '@/components/ui/resizable';

import { ArchiChatPanel } from './agent/archi-chat-panel';
import { ArchitectureCanvas } from './components/architecture-canvas';

/** 左栏（架构 Agent 面板）分栏尺寸（百分比 / rem 字面量，与 change 详情左栏同构） */
const ARCHI_PANEL_DEFAULT_SIZE = '30';
const ARCHI_PANEL_MIN_SIZE = '22';
/** 收起后留窄条（`collapsed` 时为面板宽度，窄条内呈展开入口） */
const ARCHI_PANEL_COLLAPSED_SIZE = '3rem';

/**
 * 面板分栏的命令面与折叠态：折叠态**真相取页面 state**（点击开关即同步，刻意不
 * 接分栏的 resize 回调派生）；`toggle` 依 state 分派 `collapse()` / `expand()`，
 * 页面头开关与窄条展开按钮共用同一入口；挂载时先收紧一次（layout effect 先于
 * 首帧，无可视展开闪烁）。
 */
function useArchiPanelState(): {
  panelRef: ReturnType<typeof usePanelRef>;
  collapsed: boolean;
  toggle: () => void;
} {
  const panelRef = usePanelRef();
  const [collapsed, setCollapsed] = useState(true);
  const toggle = useCallback(() => {
    const panel = panelRef.current;
    if (panel === null) return;
    if (collapsed) {
      panel.expand();
    } else {
      panel.collapse();
    }
    setCollapsed(!collapsed);
  }, [collapsed, panelRef]);
  // 默认收起：分栏挂载即落到窄条（`collapse` 记下展开尺寸 = `defaultSize`）
  useLayoutEffect(() => {
    panelRef.current?.collapse();
  }, [panelRef]);
  return { panelRef, collapsed, toggle };
}

export function ArchitectureView({ root }: { root: string | null }): React.JSX.Element {
  const [refreshTick, setRefreshTick] = useState(0);
  const { panelRef, collapsed, toggle } = useArchiPanelState();
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">架构</h2>
        <div className="flex items-center gap-2">
          <Button data-testid="archi-chat-toggle" onClick={toggle}>
            {collapsed ? '显示架构 Agent' : '隐藏架构 Agent'}
          </Button>
          <Button data-testid="archi-refresh" onClick={() => setRefreshTick((tick) => tick + 1)}>
            刷新
          </Button>
        </div>
      </div>
      <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
        <ResizablePanel
          defaultSize={ARCHI_PANEL_DEFAULT_SIZE}
          minSize={ARCHI_PANEL_MIN_SIZE}
          collapsible
          collapsedSize={ARCHI_PANEL_COLLAPSED_SIZE}
          panelRef={panelRef}
        >
          <ArchiChatPanel root={root} collapsed={collapsed} onToggleCollapsed={toggle} />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel minSize="40">
          <div className="h-full min-h-0 pl-1.5">
            <ArchitectureCanvas key={refreshTick} root={root} mode="full" />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
