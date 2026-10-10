import { open } from '@tauri-apps/plugin-dialog';
import { useCallback } from 'react';
import { HashRouter } from 'react-router';

import { AppSidebar } from '@/components/app-sidebar';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import { SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { Toaster } from '@/components/ui/sonner';
import { useUpdater, type UpdateState } from '@/hooks/use-updater';
import { useWorkspaces } from '@/hooks/use-workspaces';
import { WelcomeView } from '@/views/welcome-view';

import { AppRoutes } from './routes';

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

/** 壳态顶栏：侧栏开关 + 应用名 + 更新指示 */
function ShellHeader({ update }: { update: UpdateState }): React.JSX.Element {
  return (
    <header className="flex flex-wrap items-center gap-3 border-b border-border bg-card px-4 py-2.5">
      <SidebarTrigger />
      <Separator className="mr-2 data-[orientation=vertical]:h-4" orientation="vertical" />
      <strong>Dev Team</strong>
      <span className="flex-1" />
      <UpdateIndicator state={update} />
    </header>
  );
}

export default function App() {
  const workspaceState = useWorkspaces();
  const update = useUpdater();

  // 对话框添加流经 useWorkspaces().add 入库；成功即以返回记录的 canonical
  // root 为当前根（清单为默认序，新记录未必居首）
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
        <HashRouter>
          <SidebarProvider>
            <AppSidebar
              currentRoot={workspaceState.root}
              onAdd={pickAndAdd}
              onOpen={workspaceState.select}
              onRemove={workspaceState.remove}
              workspaces={workspaceState.workspaces}
            />
            <main className="flex-1 min-w-0 max-h-screen flex flex-col">
              <ShellHeader update={update} />
              <ScrollArea className="mx-auto flex min-h-0 w-full flex-1 flex-col px-4 py-4">
                <AppRoutes root={workspaceState.root} />
              </ScrollArea>
            </main>
          </SidebarProvider>
        </HashRouter>
      )}
      <Toaster />
    </>
  );
}
