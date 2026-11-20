import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { initialOf } from "@/lib/format"
import { cn } from "@/lib/utils"

export function DeviceAvatar({
  name,
  self = false,
  online = false,
  size = "default",
  className,
}: {
  name: string
  self?: boolean
  online?: boolean
  size?: "sm" | "default" | "lg"
  className?: string
}) {
  return (
    <div className={cn("relative shrink-0", className)}>
      <Avatar
        size={size}
        className={cn(
          "after:border-foreground/10",
          self && "after:border-transparent"
        )}
      >
        <AvatarFallback
          className={cn(
            "font-medium",
            self
              ? "bg-foreground text-background"
              : "bg-muted text-foreground/75"
          )}
        >
          {initialOf(name)}
        </AvatarFallback>
      </Avatar>
      {online && (
        <span
          className="absolute right-0 bottom-0 size-2 rounded-full bg-emerald-500 ring-2 ring-background"
          aria-label="Online"
        />
      )}
    </div>
  )
}
