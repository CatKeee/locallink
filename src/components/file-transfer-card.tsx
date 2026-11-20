import type { ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import {
  ExternalLinkIcon,
  FileArchiveIcon,
  FileAudioIcon,
  FileCode2Icon,
  FileIcon,
  FileImageIcon,
  FileSpreadsheetIcon,
  FileTextIcon,
  FileVideoIcon,
  FolderOpenIcon,
  PauseIcon,
  PlayIcon,
  RefreshCwIcon,
  XIcon,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import {
  extensionOf,
  formatBytes,
  formatEta,
  formatSpeed,
} from "@/lib/format"
import { cn } from "@/lib/utils"
import type { Transfer } from "@/types"

function fileVisual(name: string): {
  icon: LucideIcon
  className: string
} {
  const extension = extensionOf(name)
  if (["png", "jpg", "jpeg", "gif", "webp", "heic", "avif", "svg"].includes(extension)) {
    return {
      icon: FileImageIcon,
      className: "bg-sky-500/10 text-sky-600 dark:text-sky-400",
    }
  }
  if (["mp4", "mov", "mkv", "avi", "webm", "m4v"].includes(extension)) {
    return {
      icon: FileVideoIcon,
      className: "bg-violet-500/10 text-violet-600 dark:text-violet-400",
    }
  }
  if (["mp3", "wav", "flac", "aac", "m4a", "ogg"].includes(extension)) {
    return {
      icon: FileAudioIcon,
      className: "bg-pink-500/10 text-pink-600 dark:text-pink-400",
    }
  }
  if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz"].includes(extension)) {
    return {
      icon: FileArchiveIcon,
      className: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
    }
  }
  if (
    ["js", "ts", "tsx", "jsx", "json", "html", "css", "py", "rs", "go", "sh", "yml", "yaml"].includes(
      extension
    )
  ) {
    return {
      icon: FileCode2Icon,
      className: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    }
  }
  if (["xls", "xlsx", "csv", "numbers"].includes(extension)) {
    return {
      icon: FileSpreadsheetIcon,
      className: "bg-green-500/10 text-green-600 dark:text-green-400",
    }
  }
  if (["pdf", "doc", "docx", "txt", "md", "rtf", "pages"].includes(extension)) {
    return {
      icon: FileTextIcon,
      className: "bg-blue-500/10 text-blue-600 dark:text-blue-400",
    }
  }
  return {
    icon: FileIcon,
    className: "bg-muted text-muted-foreground",
  }
}

function statusLabel(transfer: Transfer): string {
  switch (transfer.state) {
    case "waiting":
      return transfer.direction === "out" ? "Waiting for recipient…" : "Preparing to receive…"
    case "transferring":
      return transfer.direction === "out" ? "Sending" : "Receiving"
    case "paused":
      return "Paused"
    case "completed":
      return transfer.direction === "out" ? "Sent" : "Received and saved"
    case "cancelled":
      return "Cancelled"
    case "failed":
      return transfer.direction === "out" ? "Send failed" : "Receive failed"
  }
}

function IconAction({
  label,
  onClick,
  children,
  destructive = false,
}: {
  label: string
  onClick: () => void
  children: ReactNode
  destructive?: boolean
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-xs"
            className={cn(
              "text-muted-foreground",
              destructive
                ? "hover:bg-destructive/10 hover:text-destructive"
                : "hover:text-foreground"
            )}
            onClick={onClick}
            aria-label={label}
          />
        }
      >
        {children}
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

export function FileTransferCard({
  transfer,
  onPause,
  onResume,
  onCancel,
  onOpen,
  onReveal,
  onRetry,
}: {
  transfer: Transfer
  onPause: (id: string) => void
  onResume: (id: string) => void
  onCancel: (id: string) => void
  onOpen: (path: string) => void
  onReveal: (path: string) => void
  onRetry: (path: string) => void
}) {
  const visual = fileVisual(transfer.name)
  const Icon = visual.icon
  const active = ["waiting", "transferring", "paused"].includes(transfer.state)
  const percent =
    transfer.size === 0
      ? transfer.state === "completed"
        ? 100
        : 0
      : Math.min(
          100,
          Math.round((transfer.transferred / transfer.size) * 100)
        )
  const failed = transfer.state === "failed"
  const completed = transfer.state === "completed"
  const incoming = transfer.direction === "in"

  return (
    <div
      className={cn(
        "w-[344px] max-w-full overflow-hidden rounded-xl border bg-card text-left shadow-xs transition-colors",
        failed && "border-destructive/30"
      )}
    >
      <div className="flex items-start gap-3 p-3">
        <div
          className={cn(
            "grid size-10 shrink-0 place-items-center rounded-lg",
            visual.className
          )}
        >
          <Icon className="size-[18px]" strokeWidth={1.7} />
        </div>

        <div className="min-w-0 flex-1 pt-0.5">
          <p
            className="truncate text-[12.5px] font-medium"
            title={transfer.name}
          >
            {transfer.name}
          </p>
          <p className="mt-1 text-[10.5px] text-muted-foreground">
            {formatBytes(transfer.size)} · {statusLabel(transfer)}
          </p>
        </div>

        <div className="flex shrink-0 items-center gap-0.5">
          {transfer.state === "transferring" && percent < 100 && (
            <IconAction label="Pause" onClick={() => onPause(transfer.id)}>
              <PauseIcon />
            </IconAction>
          )}
          {transfer.state === "paused" && (
            <IconAction label="Resume" onClick={() => onResume(transfer.id)}>
              <PlayIcon />
            </IconAction>
          )}
          {active && (
            <IconAction
              label="Cancel transfer"
              destructive
              onClick={() => onCancel(transfer.id)}
            >
              <XIcon />
            </IconAction>
          )}
          {completed && transfer.path && incoming && (
            <>
              <IconAction
                label="Open file"
                onClick={() => onOpen(transfer.path!)}
              >
                <ExternalLinkIcon />
              </IconAction>
              <IconAction
                label="Show in folder"
                onClick={() => onReveal(transfer.path!)}
              >
                <FolderOpenIcon />
              </IconAction>
            </>
          )}
          {completed && transfer.path && !incoming && (
            <IconAction
              label="Show in folder"
              onClick={() => onReveal(transfer.path!)}
            >
              <FolderOpenIcon />
            </IconAction>
          )}
          {(transfer.state === "failed" ||
            transfer.state === "cancelled") &&
            !incoming &&
            transfer.path && (
              <IconAction
                label="Send again"
                onClick={() => onRetry(transfer.path!)}
              >
                <RefreshCwIcon />
              </IconAction>
            )}
        </div>
      </div>

      {active && (
        <div className="px-3 pb-3">
          <Progress value={percent} className="gap-0" />
          <div className="mt-2 flex items-center justify-between gap-3 text-[10px] text-muted-foreground tabular-nums">
            <span className="truncate">
              {percent}%
              {transfer.state === "transferring" && transfer.speedBps > 0
                ? ` · ${formatSpeed(transfer.speedBps)}`
                : ""}
            </span>
            <span className="shrink-0">
              {transfer.state === "waiting"
                ? "Waiting"
                : transfer.state === "paused"
                  ? "Paused"
                  : percent >= 100
                    ? "Finishing"
                    : transfer.etaSecs
                      ? `Remaining ${formatEta(transfer.etaSecs)}`
                      : "Calculating"}
            </span>
          </div>
        </div>
      )}

      {failed && transfer.error && (
        <p className="border-t border-destructive/10 bg-destructive/[0.03] px-3 py-2 text-[10.5px] leading-4 text-destructive/90">
          {transfer.error}
        </p>
      )}

      {completed && incoming && (
        <div className="flex items-center gap-1.5 border-t bg-muted/20 px-3 py-2 text-[10px] text-muted-foreground">
          <FolderOpenIcon className="size-3" />
          Saved to Downloads
        </div>
      )}
    </div>
  )
}
