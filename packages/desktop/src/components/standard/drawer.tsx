import { Button } from '@/components/ui/button';
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from '@/components/ui/drawer';

import { useOpen } from './hooks/use-open';

type StandardDrawerProps = {
  title: string;
  content: React.JSX.Element;
  containerClassName?: string;
  trigger?: React.JSX.Element;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** 模态开关（缺省 `true`：遮罩 + 点遮罩关闭）；非模态时无遮罩、容器不吃
   * 指针事件（页面其余区域仍可交互）。`modal` 只透传底层 `Drawer`，不进
   * `DrawerContent` 的 rest（避免多出未知 DOM 属性） */
  modal?: boolean;
};

export function StandardDrawer({
  title,
  content,
  containerClassName,
  trigger,
  open: outerOpen,
  onOpenChange: onOuterOpenChange,
  modal = true,
  ...rest
}: StandardDrawerProps) {
  const hasTrigger = trigger !== undefined;
  const [open, setOpen] = useOpen(outerOpen, onOuterOpenChange);

  return (
    <Drawer swipeDirection="right" modal={modal} open={open} onOpenChange={setOpen}>
      {hasTrigger && <DrawerTrigger render={trigger} />}
      <DrawerContent className={containerClassName} {...rest}>
        <DrawerHeader>
          <DrawerTitle>{title}</DrawerTitle>
        </DrawerHeader>
        {content}
        <DrawerFooter>
          <DrawerClose render={<Button variant="outline" />}>关闭</DrawerClose>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
