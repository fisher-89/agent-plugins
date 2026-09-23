import { Plus } from 'lucide-react';

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

export interface AppSidebarProps {
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
            tooltip={record.root}
            onClick={() => onOpen(record.root)}
          >
            <span className="flex min-w-0 flex-col">
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

/**
 * workspace 清单侧栏（仅壳态挂载）：单「工作区」组呈现全部清单。
 * 列表项点击切换（副文本父目录区分同名、Tooltip 完整 root），「＋」添加、
 * 右键 ContextMenu 移除。
 */
export function AppSidebar({
  workspaces,
  currentRoot,
  onOpen,
  onAdd,
  onRemove,
}: AppSidebarProps): React.JSX.Element {
  return (
    <Sidebar collapsible="icon">
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
