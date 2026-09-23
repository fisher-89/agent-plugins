import { open } from '@tauri-apps/plugin-dialog';
import { useCallback } from 'react';

import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { SidebarInset, SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { Toaster } from '@/components/ui/sonner';

import { AppSidebar } from './components/AppSidebar';
import { useChangeList } from './hooks/useChangeList';
import { useUpdater, type UpdateState } from './hooks/useUpdater';
import { useWorkspaces } from './hooks/useWorkspaces';
import { ChangeView } from './views/changes/ChangeView';
import { WelcomeView } from './views/WelcomeView';

/**
 * 顶栏更新指示：当前版本号 + 更新入口。有新版本 →「更新到 vX」→ 下载进度 →
 * Finished 转「正在安装…」终态（Windows 上 NSIS 安装器接管重启，其后无回调）；
 * 失败 →「重试更新」。
 */
function UpdateIndicator({ state }: { state: UpdateState }) {
  return (
    <>
      {state.currentVersion !== null && (
        <span className="text-xs text-muted-foreground">v{state.currentVersion}</span>
      )}
      {state.status === 'installing' ? (
        <span className="text-xs text-muted-foreground">正在安装…</span>
      ) : state.status === 'downloading' ? (
        <span className="text-xs text-muted-foreground">下载中 {state.progress ?? 0}%</span>
      ) : state.error !== null ? (
        <Button onClick={state.start}>重试更新</Button>
      ) : (
        state.available !== null && (
          <Button onClick={state.start}>更新到 v{state.available.version}</Button>
        )
      )}
    </>
  );
}

/**
 * 应用壳：workspace 选择与恢复（启动自动恢复收在 useWorkspaces；清单切换/添加/
 * 移除收在 AppSidebar）+ 列表 / 详情视图切换。root === null 停欢迎屏，此时不渲染
 * SidebarProvider / 侧栏 DOM；<Toaster /> 与条件渲染同级置于 App 根，欢迎态/壳态
 * 均覆盖。组件不直接 invoke，取数统一经 useWorkspaces / useChangeList / useChangeDetail。
 */
export default function App() {
  const workspaceState = useWorkspaces();
  const list = useChangeList(workspaceState.root);
  const update = useUpdater();

  // 对话框添加流经 useWorkspaces().add 入库；刷新后新记录 last_opened_at 最新，
  // 即清单第一名，root 随之切换到返回记录的 canonical root
  const pickAndAdd = useCallback(async () => {
    let selected: unknown;
    try {
      selected = await open({ directory: true, multiple: false });
    } catch {
      return; // 对话框调用失败：保持现状，不中断应用
    }
    if (typeof selected !== 'string') return; // 取消选择则保持现状
    const record = await workspaceState.add(selected);
    if (record === null) return; // 入库失败：toast 已呈现，不打开
  }, [workspaceState]);

  return (
    <>
      {workspaceState.root === null ? (
        <WelcomeView state={workspaceState} onAdd={pickAndAdd} />
      ) : (
        <SidebarProvider>
          <AppSidebar
            currentRoot={workspaceState.root}
            onAdd={pickAndAdd}
            onOpen={workspaceState.touch}
            onRemove={workspaceState.remove}
            workspaces={workspaceState.workspaces}
          />
          <SidebarInset>
            <header className="flex flex-wrap items-center gap-3 border-b border-border bg-card px-4 py-2.5">
              <SidebarTrigger />
              <Separator className="mr-2 data-[orientation=vertical]:h-4" orientation="vertical" />
              <strong>Dev Team</strong>
              <span className="flex-1" />
              <UpdateIndicator state={update} />
            </header>
            <div className="mx-auto w-full max-w-[1100px] flex-1 px-4 py-4">
              <ChangeView root={workspaceState.root} list={list} />
            </div>
          </SidebarInset>
        </SidebarProvider>
      )}
      <Toaster />
    </>
  );
}
