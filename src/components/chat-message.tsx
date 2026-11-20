import { CopyIcon } from "lucide-react"

import { FileTransferCard } from "@/components/file-transfer-card"
import { formatTime } from "@/lib/format"
import { cn } from "@/lib/utils"
import type { ChatMessage, Transfer } from "@/types"

export function ChatMessageRow({
  message,
  transfer,
  grouped,
  onCopy,
  onPause,
  onResume,
  onCancel,
  onOpen,
  onReveal,
  onRetry,
}: {
  message: ChatMessage
  transfer: Transfer | null
  grouped: boolean
  onCopy: (text: string) => void
  onPause: (id: string) => void
  onResume: (id: string) => void
  onCancel: (id: string) => void
  onOpen: (path: string) => void
  onReveal: (path: string) => void
  onRetry: (path: string) => void
}) {
  const outgoing = message.direction === "out"
  const text = message.text ?? ""
  const isFile = message.kind === "file" && transfer

  return (
    <div
      className={cn(
        "group/message flex w-full min-w-0 gap-2.5",
        outgoing && "flex-row-reverse",
        grouped ? "mt-1" : "mt-4 first:mt-0"
      )}
    >
      <div
        className={cn(
          "flex min-w-0 max-w-[78%] flex-col gap-1",
          outgoing ? "items-end" : "items-start"
        )}
      >
        {isFile && transfer ? (
          <FileTransferCard
            transfer={transfer}
            onPause={onPause}
            onResume={onResume}
            onCancel={onCancel}
            onOpen={onOpen}
            onReveal={onReveal}
            onRetry={onRetry}
          />
        ) : (
          <div
            className={cn(
              "max-w-full overflow-hidden rounded-2xl border px-3.5 py-2.5 text-[13px] leading-[1.65] shadow-xs",
              outgoing
                ? "rounded-br-md border-transparent bg-primary text-primary-foreground"
                : "rounded-tl-md bg-muted/70 text-foreground"
            )}
          >
            <p className="break-words whitespace-pre-wrap [overflow-wrap:anywhere]">
              {text}
            </p>
          </div>
        )}

        <div
          className={cn(
            "flex items-center gap-2 px-1 text-[10px] text-muted-foreground",
            outgoing && "flex-row-reverse"
          )}
        >
          <span className="tabular-nums">{formatTime(message.at)}</span>
          {!isFile && (
            <button
              type="button"
              onClick={() => onCopy(text)}
              className="inline-flex items-center gap-1 rounded-sm opacity-0 transition-opacity outline-none group-hover/message:opacity-100 hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring/40"
            >
              <CopyIcon className="size-2.5" />
              Copy
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
