import { open } from '@tauri-apps/plugin-dialog';
import { useCallback } from 'react';
import { HashRouter } from 'react-router';

import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { SidebarInset, SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { Toaster } from '@/components/ui/sonner';

import { AppSidebar } from './components/AppSidebar';
import { useChangeList } from './hooks/useChangeList';
import { useUpdater, type UpdateState } from './hooks/useUpdater';
import { useWorkspaces } from './hooks/useWorkspaces';
import { AppRoutes } from './routes';
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

/**
 * 应用壳：workspace 选择与恢复（启动自动恢复收在 useWorkspaces；清单切换/添加/
 * 移除收在 AppSidebar）+ 路由化顶层导航（HashRouter 内 AppRoutes 路由表，
 * change 选中由 /changes/:name 路由参数承载）。root === null 停欢迎屏，此时
 * 不挂 Router、不渲染 SidebarProvider / 侧栏 DOM；<Toaster /> 与条件渲染同级
 * 置于 App 根，欢迎态/壳态均覆盖。组件不直接 invoke，取数统一经 useWorkspaces /
 * useChangeList / useChangeDetail / agent 域 hooks。切页（路由切换）后 ChangeView
 * 卸载、选中 change 随 URL 消失（清单数据留 App 层不丢）；agent 页 remount 后
 * 经历史重放呈现已有内容。
 */
export default function App() {
  const workspaceState = useWorkspaces();
  const list = useChangeList(workspaceState.root);
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
            <SidebarInset>
              <ShellHeader update={update} />
              <div className="mx-auto w-full max-w-[1100px] flex-1 px-4 py-4">
                <AppRoutes list={list} root={workspaceState.root} />
              </div>
            </SidebarInset>
          </SidebarProvider>
        </HashRouter>
      )}
      <Toaster />
    </>
  );
}
