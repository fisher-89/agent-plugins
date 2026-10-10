import {
  Bot,
  Boxes,
  ChevronsUpDown,
  Compass,
  Database,
  GitBranch,
  Info,
  PlusIcon,
  Settings,
} from 'lucide-react';
import { NavLink, useLocation } from 'react-router';

import {
  Sidebar,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';

import type { WorkspaceRecord } from '../types/dto';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';

export interface AppSidebarProps {
  /** workspace 清单 */
  workspaces: WorkspaceRecord[];
  /** 当前选中根（启动恢复/移除顺延取默认序第一名，切换为本地 state） */
  currentRoot: string;
  /** 点击清单项 */
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

/** 页面导航单项：SidebarMenuItem + NavLink 路由入口（active 由当前 URL 派生） */
function PageNavItem({
  to,
  testId,
  icon,
  label,
  active,
}: {
  to: string;
  testId: string;
  icon: React.JSX.Element;
  label: string;
  active: boolean;
}): React.JSX.Element {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton
        isActive={active}
        tooltip={label}
        render={
          <NavLink data-testid={testId} to={to}>
            {icon}
            <span>{label}</span>
          </NavLink>
        }
      />
    </SidebarMenuItem>
  );
}

/** 页面导航组：[基础信息] [变更] [探索] [配置]，NavLink 路由入口（active 由当前 URL 派生） */
function PageNavGroup(): React.JSX.Element {
  const { pathname } = useLocation();
  return (
    <>
      {/* 基础信息项：/info（workspace 域内容页首位） */}
      <PageNavItem
        active={pathname === '/info'}
        icon={<Info />}
        label="基础信息"
        testId="nav-info"
        to="/info"
      />
      {/* 变更项：/changes 与 /changes/:id（详情）均 active */}
      <PageNavItem
        active={pathname === '/changes' || pathname.startsWith('/changes/')}
        icon={<GitBranch />}
        label="变更"
        testId="nav-changes"
        to="/changes"
      />
      {/* 探索项：/explores 与 /explores/:name（详情）均 active */}
      <PageNavItem
        active={pathname === '/explores' || pathname.startsWith('/explores/')}
        icon={<Compass />}
        label="探索"
        testId="nav-explores"
        to="/explores"
      />
      {/* 配置项：/config（workspace 域内容页组内末位） */}
      <PageNavItem
        active={pathname === '/config'}
        icon={<Settings />}
        label="配置"
        testId="nav-config"
        to="/config"
      />
    </>
  );
}

/** 系统工具导航组：[Agent 管理]（调试的配置前置）[Agent 调试] [数据库]，系统级工具入口 */
function SystemToolsGroup(): React.JSX.Element {
  const { pathname } = useLocation();
  return (
    <SidebarGroup>
      <SidebarGroupLabel>系统工具</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={pathname === '/agents'}
              tooltip="Agent 管理"
              render={
                <NavLink data-testid="nav-agents" to="/agents">
                  <Boxes />
                  <span>Agent 管理</span>
                </NavLink>
              }
            />
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={pathname === '/agent'}
              tooltip="Agent 调试"
              render={
                <NavLink data-testid="nav-agent" to="/agent">
                  <Bot />
                  <span>Agent 调试</span>
                </NavLink>
              }
            />
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={pathname === '/db'}
              tooltip="数据库"
              render={
                <NavLink data-testid="nav-db" to="/db">
                  <Database />
                  <span>数据库</span>
                </NavLink>
              }
            />
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

/** 工作区触发按钮内容：当前记录名 + 父目录末段（空父目录不渲染副行）。 */
function WorkspaceTriggerLabel({
  record,
  parent,
}: {
  record: WorkspaceRecord;
  parent: string;
}): React.JSX.Element {
  return (
    <>
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
      <ChevronsUpDown className="ml-auto" />
    </>
  );
}

/** 工作区下拉清单：切换（逐条）与添加入口（分隔组）。 */
function WorkspaceMenu({
  workspaces,
  onOpen,
  onAdd,
}: {
  workspaces: WorkspaceRecord[];
  onOpen: (root: string) => void;
  onAdd: () => void;
}): React.JSX.Element {
  return (
    <DropdownMenuContent side="right">
      <DropdownMenuGroup>
        <DropdownMenuLabel>工作区</DropdownMenuLabel>
        {workspaces.map((record) => (
          <DropdownMenuItem key={record.root} onClick={() => onOpen(record.root)}>
            {record.name}
          </DropdownMenuItem>
        ))}
      </DropdownMenuGroup>
      <DropdownMenuSeparator />
      <DropdownMenuGroup>
        <DropdownMenuItem onClick={onAdd}>
          <PlusIcon />
          添加工作区
        </DropdownMenuItem>
      </DropdownMenuGroup>
    </DropdownMenuContent>
  );
}

function WorkspaceSelector({
  workspaces,
  currentRoot,
  onOpen,
  onAdd,
}: {
  workspaces: WorkspaceRecord[];
  currentRoot: string;
  onOpen: (root: string) => void;
  onAdd: () => void;
  onRemove: (root: string) => void;
}) {
  if (workspaces.length === 0) {
    return (
      <SidebarMenuItem>
        <SidebarMenuButton size="lg" onClick={onAdd}>
          <span className="flex min-w-0 flex-col leading-tight">
            <span>未关联工作区</span>
            <span>点击添加</span>
          </span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    );
  }
  const currentRecord = workspaces.find((record) => record.root === currentRoot);
  if (!currentRecord) {
    return null;
  }
  const parent = parentDir(currentRecord.root);
  return (
    <SidebarMenuItem>
      <DropdownMenu>
        <DropdownMenuTrigger render={<SidebarMenuButton size="lg" />}>
          <WorkspaceTriggerLabel record={currentRecord} parent={parent} />
        </DropdownMenuTrigger>
        <WorkspaceMenu workspaces={workspaces} onOpen={onOpen} onAdd={onAdd} />
      </DropdownMenu>
    </SidebarMenuItem>
  );
}

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
        <SidebarGroupContent>
          <SidebarMenu>
            <WorkspaceSelector
              workspaces={workspaces}
              currentRoot={currentRoot}
              onOpen={onOpen}
              onAdd={onAdd}
              onRemove={onRemove}
            />
            {currentRoot && <PageNavGroup />}
          </SidebarMenu>
        </SidebarGroupContent>
      </SidebarGroup>
      <SystemToolsGroup />
    </Sidebar>
  );
}
