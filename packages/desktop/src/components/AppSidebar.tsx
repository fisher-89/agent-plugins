import { Bot, GitBranch, Plus } from 'lucide-react';

import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/components/ui/context-menu';
import {
  Sidebar,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';

import type { WorkspaceRecord } from '../types/dto';

/** 顶层视图判别（App 与 Sidebar 共用；本地 state 切换，无路由） */
export type TopPage = 'changes' | 'agent';

export interface AppSidebarProps {
  /** 当前顶层视图（缺省 changes） */
  page?: TopPage;
  /** 页面导航组点击切换 */
  onPageChange?: (page: TopPage) => void;
  /** workspace 清单（后端按 last_opened_at 降序） */
  workspaces: WorkspaceRecord[];
  /** 当前选中根（清单第一名） */
  currentRoot: string;
  /** 点击清单项：touch → 清单重排 → 恒取第一名 */
  onOpen: (root: string) => void;
  /** 「＋」添加：文件夹选择器 → add_workspace 入库 */
  onAdd: () => void;
  /** 右键「移除」：remove_workspace，无确认弹窗、不触盘上目录 */
  onRemove: (root: string) => void;
}

/** root 去掉最后一段的完整前缀（最后一个 / 或 \ 之前）；无分隔符返回空串（design D3） */
function parentDir(root: string): string {
  const index = Math.max(root.lastIndexOf('/'), root.lastIndexOf('\\'));
  return index === -1 ? '' : root.slice(0, index);
}

function WorkspaceItem({
  record,
  currentRoot,
  onOpen,
  onRemove,
}: {
  record: WorkspaceRecord;
  currentRoot: string;
  onOpen: (root: string) => void;
  onRemove: (root: string) => void;
}): React.JSX.Element {
  const parent = parentDir(record.root);
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <SidebarMenuItem>
          <SidebarMenuButton
            data-root={record.root}
            data-testid="workspace-item"
            isActive={record.root === currentRoot}
            size="lg"
            tooltip={record.root}
            onClick={() => onOpen(record.root)}
          >
            {/* leading-tight 收紧行高，主文本 + 父目录两行在 lg（h-12）内完整呈现 */}
            <span className="flex min-w-0 flex-col leading-tight">
              <span className="truncate">{record.name}</span>
              {parent !== '' && (
                <span
                  className="block truncate text-xs font-normal text-muted-foreground"
                  data-testid="workspace-sub"
                >
                  {parent}
                </span>
              )}
            </span>
          </SidebarMenuButton>
        </SidebarMenuItem>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onClick={() => onRemove(record.root)}>移除</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

/** 页面导航组：[变更] [Agent 调试]，顶层视图切换入口（无路由） */
function PageNavGroup({
  page,
  onPageChange,
}: {
  page: TopPage;
  onPageChange?: (page: TopPage) => void;
}): React.JSX.Element {
  return (
    <SidebarGroup>
      <SidebarGroupLabel>页面</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              data-testid="nav-changes"
              isActive={page === 'changes'}
              tooltip="变更"
              onClick={() => onPageChange?.('changes')}
            >
              <GitBranch />
              <span>变更</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              data-testid="nav-agent"
              isActive={page === 'agent'}
              tooltip="Agent 调试"
              onClick={() => onPageChange?.('agent')}
            >
              <Bot />
              <span>Agent 调试</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

/**
 * 侧栏（仅壳态挂载）：上方「页面」导航组（[变更] [Agent 调试]，首次出现
 * 非 workspace 入口语义），下方「工作区」清单组语义不变。列表项点击切换
 * （副文本父目录区分同名、Tooltip 完整 root），「＋」添加、右键 ContextMenu
 * 移除。
 */
export function AppSidebar({
  page = 'changes',
  onPageChange,
  workspaces,
  currentRoot,
  onOpen,
  onAdd,
  onRemove,
}: AppSidebarProps): React.JSX.Element {
  return (
    <Sidebar collapsible="icon">
      <PageNavGroup page={page} onPageChange={onPageChange} />
      <SidebarGroup>
        <SidebarGroupLabel>工作区</SidebarGroupLabel>
        <SidebarGroupAction aria-label="添加 workspace" onClick={onAdd}>
          <Plus />
        </SidebarGroupAction>
        <SidebarGroupContent>
          <SidebarMenu>
            {workspaces.map((record) => (
              <WorkspaceItem
                currentRoot={currentRoot}
                key={record.root}
                onOpen={onOpen}
                onRemove={onRemove}
                record={record}
              />
            ))}
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
    </Sidebar>
  );
}
