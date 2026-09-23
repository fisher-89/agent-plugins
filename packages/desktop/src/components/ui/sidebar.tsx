import { cva } from 'class-variance-authority';
import { PanelLeftIcon } from 'lucide-react';
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { useIsMobile } from '@/hooks/use-mobile';
import { cn } from '@/lib/utils';

/* shadcn Sidebar vendored 内部化（裁剪版）：导出集 pin 为本 app 实际用量
   （Provider / Sidebar / Trigger / Inset / Group 系列 / Menu 系列；useSidebar 仅内部消费）。
   配色映射 app 既有 token（bg-card / border-border / text-muted-foreground），不引入
   shadcn sidebar 主题组。折叠态会话内 state（design D1：去 cookie 持久化，defaultOpen
   固定 true）；collapsible="icon" + 内建 Ctrl/Cmd+B 保留；TooltipProvider
   delayDuration={0}（D4）；菜单项 Tooltip 不限定折叠态才显示（D3 信息层级：副文本=父
   目录、Tooltip=完整 root）。未用子组件（MenuSkeleton / MenuSub / Input / Rail 等）与
   side / variant 分支按 D5 删减。 */

const SIDEBAR_WIDTH = '16rem';
const SIDEBAR_WIDTH_ICON = '3rem';
const SIDEBAR_WIDTH_MOBILE = '18rem';
const SIDEBAR_KEYBOARD_SHORTCUT = 'b';

type SidebarState = 'expanded' | 'collapsed';

interface SidebarContextValue {
  state: SidebarState;
  open: boolean;
  setOpen: (open: boolean) => void;
  openMobile: boolean;
  setOpenMobile: (open: boolean) => void;
  isMobile: boolean;
  toggleSidebar: () => void;
}

const SidebarContext = createContext<SidebarContextValue | null>(null);

/** 仅生成件内部消费，不在导出集（design pin） */
function useSidebar(): SidebarContextValue {
  const context = useContext(SidebarContext);
  if (!context) {
    throw new Error('useSidebar must be used within a SidebarProvider.');
  }
  return context;
}

/** 内建 Ctrl/Cmd+B 折叠快捷键 */
function useSidebarShortcut(toggleSidebar: () => void): void {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === SIDEBAR_KEYBOARD_SHORTCUT && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        toggleSidebar();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [toggleSidebar]);
}

interface SidebarOpenState {
  open: boolean;
  setOpen: (open: boolean) => void;
  openMobile: boolean;
  setOpenMobile: (open: boolean) => void;
  isMobile: boolean;
  toggleSidebar: () => void;
}

/** 折叠态会话内 state（D1：无 cookie / localStorage 持久化，defaultOpen 固定 true）；
 * desktop 走 open（icon 折叠），mobile 走 openMobile（Sheet 抽屉第三态） */
function useSidebarOpenState(
  defaultOpen: boolean,
  openProp: boolean | undefined,
  onOpenChangeProp: ((open: boolean) => void) | undefined,
): SidebarOpenState {
  const isMobile = useIsMobile();
  const [openMobile, setOpenMobile] = useState(false);
  const [openState, setOpenState] = useState(defaultOpen);
  const open = openProp ?? openState;
  const setOpen = useCallback(
    (value: boolean) => {
      if (onOpenChangeProp) onOpenChangeProp(value);
      else setOpenState(value);
    },
    [onOpenChangeProp],
  );
  const toggleSidebar = useCallback(
    () => (isMobile ? setOpenMobile((v) => !v) : setOpen(!open)),
    [isMobile, open, setOpen],
  );
  useSidebarShortcut(toggleSidebar);
  return { open, setOpen, openMobile, setOpenMobile, isMobile, toggleSidebar };
}

function SidebarProvider({
  defaultOpen = true,
  open: openProp,
  onOpenChange: onOpenChangeProp,
  className,
  style,
  children,
  ...props
}: React.ComponentProps<'div'> & {
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}): React.JSX.Element {
  const sidebar = useSidebarOpenState(defaultOpen, openProp, onOpenChangeProp);
  const value = useMemo<SidebarContextValue>(
    () => ({ state: sidebar.open ? 'expanded' : 'collapsed', ...sidebar }),
    [sidebar],
  );

  return (
    <SidebarContext.Provider value={value}>
      <TooltipProvider delayDuration={0}>
        <div
          data-slot="sidebar-wrapper"
          className={cn('group/sidebar-wrapper flex min-h-svh w-full', className)}
          style={
            {
              '--sidebar-width': SIDEBAR_WIDTH,
              '--sidebar-width-icon': SIDEBAR_WIDTH_ICON,
              ...style,
            } as React.CSSProperties
          }
          {...props}
        >
          {children}
        </div>
      </TooltipProvider>
    </SidebarContext.Provider>
  );
}

/** 抽屉宽度经 CSS 变量注入（w-(--sidebar-width) 消费）；显式标注免去断言 */
const SIDEBAR_MOBILE_STYLE: React.CSSProperties & Record<'--sidebar-width', string> = {
  '--sidebar-width': SIDEBAR_WIDTH_MOBILE,
};

/** mobile 第三态：Sheet 抽屉（左向），sr-only 标题供 Radix Dialog a11y */
function SidebarMobile({
  children,
  ...props
}: React.ComponentProps<typeof Sheet>): React.JSX.Element {
  const { openMobile, setOpenMobile } = useSidebar();
  return (
    <Sheet onOpenChange={setOpenMobile} open={openMobile} {...props}>
      <SheetContent
        className="w-(--sidebar-width) border-border bg-card p-0 text-foreground [&>button]:hidden"
        data-mobile="true"
        data-sidebar="sidebar"
        data-slot="sidebar"
        side="left"
        style={SIDEBAR_MOBILE_STYLE}
      >
        <SheetHeader className="sr-only">
          <SheetTitle>Sidebar</SheetTitle>
          <SheetDescription>Displays the mobile sidebar.</SheetDescription>
        </SheetHeader>
        <div className="flex h-full w-full flex-col">{children}</div>
      </SheetContent>
    </Sheet>
  );
}

function Sidebar({
  collapsible = 'icon',
  className,
  children,
  ...props
}: React.ComponentProps<'div'> & {
  collapsible?: 'icon';
}): React.JSX.Element {
  const { isMobile, state } = useSidebar();
  if (isMobile) {
    return <SidebarMobile {...props}>{children}</SidebarMobile>;
  }
  return (
    <div
      className="group peer text-foreground md:block"
      data-collapsible={state === 'collapsed' ? collapsible : ''}
      data-side="left"
      data-slot="sidebar"
      data-state={state}
    >
      {/* 占位列：宽度随折叠态过渡，推动内容区 */}
      <div
        data-slot="sidebar-gap"
        className="relative w-(--sidebar-width) bg-transparent transition-[width] duration-200 ease-linear group-data-[collapsible=icon]:w-(--sidebar-width-icon) group-data-[collapsible=icon]:overflow-hidden"
      />
      <div
        data-slot="sidebar-container"
        className={cn(
          'fixed inset-y-0 left-0 z-10 hidden h-svh w-(--sidebar-width) transition-[width] duration-200 ease-linear md:flex group-data-[collapsible=icon]:w-(--sidebar-width-icon) group-data-[collapsible=icon]:overflow-hidden',
          className,
        )}
        {...props}
      >
        <div
          data-sidebar="sidebar"
          data-slot="sidebar-inner"
          className="flex h-full w-full flex-col border-r border-border bg-card"
        >
          {children}
        </div>
      </div>
    </div>
  );
}

function SidebarTrigger({
  className,
  onClick,
  ...props
}: React.ComponentProps<typeof Button>): React.JSX.Element {
  const { toggleSidebar } = useSidebar();
  return (
    <Button
      data-sidebar="trigger"
      data-slot="sidebar-trigger"
      className={cn(
        'size-7 border-0 bg-transparent p-0 text-muted-foreground hover:bg-primary/10 hover:text-primary',
        className,
      )}
      onClick={(event) => {
        onClick?.(event);
        toggleSidebar();
      }}
      {...props}
    >
      <PanelLeftIcon />
      <span className="sr-only">Toggle Sidebar</span>
    </Button>
  );
}

function SidebarInset({ className, ...props }: React.ComponentProps<'main'>): React.JSX.Element {
  return (
    <main
      data-slot="sidebar-inset"
      className={cn('relative flex w-full flex-1 flex-col bg-background', className)}
      {...props}
    />
  );
}

function SidebarGroup({ className, ...props }: React.ComponentProps<'div'>): React.JSX.Element {
  return (
    <div
      data-sidebar="group"
      data-slot="sidebar-group"
      className={cn('relative flex w-full min-w-0 flex-col p-2', className)}
      {...props}
    />
  );
}

function SidebarGroupLabel({
  className,
  ...props
}: React.ComponentProps<'div'>): React.JSX.Element {
  return (
    <div
      data-sidebar="group-label"
      data-slot="sidebar-group-label"
      className={cn(
        'flex h-8 shrink-0 items-center gap-2 rounded-md px-2 text-xs font-medium text-muted-foreground outline-none',
        className,
      )}
      {...props}
    />
  );
}

function SidebarGroupAction({
  className,
  tooltip,
  ...props
}: React.ComponentProps<'button'> & { tooltip?: string }): React.JSX.Element {
  const button = (
    <button
      data-sidebar="group-action"
      data-slot="sidebar-group-action"
      className={cn(
        'absolute top-3.5 right-3 flex aspect-square w-5 items-center justify-center rounded-md border-0 bg-transparent p-0 text-foreground outline-none transition-transform hover:bg-primary/10 hover:text-primary focus-visible:ring-2 focus-visible:ring-primary/50 disabled:pointer-events-none disabled:opacity-50 [&>svg]:size-4 [&>svg]:shrink-0 after:absolute after:-inset-2 md:after:hidden group-data-[collapsible=icon]:hidden',
        className,
      )}
      {...props}
    />
  );
  if (!tooltip) return button;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right">{tooltip}</TooltipContent>
    </Tooltip>
  );
}

function SidebarGroupContent({
  className,
  ...props
}: React.ComponentProps<'div'>): React.JSX.Element {
  return (
    <div
      data-sidebar="group-content"
      data-slot="sidebar-group-content"
      className={cn('w-full text-sm', className)}
      {...props}
    />
  );
}

function SidebarMenu({ className, ...props }: React.ComponentProps<'ul'>): React.JSX.Element {
  return (
    <ul
      data-sidebar="menu"
      data-slot="sidebar-menu"
      className={cn('flex w-full min-w-0 flex-col gap-1', className)}
      {...props}
    />
  );
}

function SidebarMenuItem({ className, ...props }: React.ComponentProps<'li'>): React.JSX.Element {
  return (
    <li
      data-sidebar="menu-item"
      data-slot="sidebar-menu-item"
      className={cn('group/menu-item relative', className)}
      {...props}
    />
  );
}

/* 变体集按本 app 实际用量裁剪（仅 default，同 ui/button 口径）；
   悬停/激活走 primary 弱化底色，data-active 由 isActive prop 派发 */
const sidebarMenuButtonVariants = cva(
  'peer/menu-button flex w-full justify-between items-center gap-2 overflow-hidden rounded-md p-2 text-left text-sm outline-none transition-[width,height,padding] hover:bg-primary/10 hover:text-primary focus-visible:ring-2 focus-visible:ring-primary/50 active:bg-primary/10 active:text-primary disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-[active=true]:bg-primary/10 data-[active=true]:font-medium data-[active=true]:text-primary group-data-[collapsible=icon]:size-8 group-data-[collapsible=icon]:p-2 [&>span]:min-w-0 [&>span:last-child]:truncate [&>svg]:size-4 [&>svg]:shrink-0',
  {
    variants: {
      variant: {
        default: 'bg-transparent text-foreground',
      },
      size: {
        default: 'h-7 text-sm',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
);

function SidebarMenuButton({
  isActive = false,
  tooltip,
  className,
  ...props
}: React.ComponentProps<typeof Button> & {
  isActive?: boolean;
  tooltip?: string | React.ReactElement;
}): React.JSX.Element {
  const button = (
    <Button
      data-active={isActive}
      data-sidebar="menu-button"
      data-size="default"
      data-slot="sidebar-menu-button"
      className={cn(sidebarMenuButtonVariants(), className)}
      {...props}
    />
  );
  if (!tooltip) return button;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right">{tooltip}</TooltipContent>
    </Tooltip>
  );
}

export {
  Sidebar,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
};
