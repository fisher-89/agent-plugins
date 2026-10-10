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
};

export function StandardDrawer({
  title,
  content,
  containerClassName,
  trigger,
  open: outerOpen,
  onOpenChange: onOuterOpenChange,
  ...rest
}: StandardDrawerProps) {
  const hasTrigger = trigger !== undefined;
  const [open, setOpen] = useOpen(outerOpen, onOuterOpenChange);

  return (
    <Drawer swipeDirection="right" open={open} onOpenChange={setOpen}>
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
