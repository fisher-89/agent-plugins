import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

interface TextTooltipProps {
  children: React.JSX.Element;
  text: string;
}

export function TextTooltip({ children, text }: TextTooltipProps) {
  return (
    <Tooltip>
      <TooltipTrigger render={children} />
      <TooltipContent>{text}</TooltipContent>
    </Tooltip>
  );
}
