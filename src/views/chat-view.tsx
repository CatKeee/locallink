import { useEffect, useRef, useState } from "react"
import { getCurrentWebview } from "@tauri-apps/api/webview"
import type { UnlistenFn } from "@tauri-apps/api/event"
import {
  CheckIcon,
  LockKeyholeIcon,
  ShieldCheckIcon,
  UnplugIcon,
  UploadCloudIcon,
} from "lucide-react"

import { ChatComposer } from "@/components/chat-composer"
import { ChatMessageRow } from "@/components/chat-message"
import { DisconnectDialog } from "@/components/connection-dialogs"
import { ModeToggle } from "@/components/mode-toggle"
import { Button } from "@/components/ui/button"
import { formatDuration, platformLabel } from "@/lib/format"
import type { ChatMessage, SessionInfo, Transfer } from "@/types"

export function ChatView({
  session,
  messages,
  transfers,
  onDisconnect,
  onSendText,
  onSendFiles,
  onPauseTransfer,
  onResumeTransfer,
  onCancelTransfer,
  onOpenPath,
  onRevealPath,
  onCopyText,
}: {
  session: SessionInfo
  messages: ChatMessage[]
  transfers: Record<string, Transfer>
  onDisconnect: () => Promise<void>
  onSendText: (text: string) => Promise<boolean>
  onSendFiles: (paths: string[]) => Promise<void>
  onPauseTransfer: (id: string) => Promise<void>
  onResumeTransfer: (id: string) => Promise<void>
  onCancelTransfer: (id: string) => Promise<void>
  onOpenPath: (path: string) => Promise<void>
  onRevealPath: (path: string) => Promise<void>
  onCopyText: (text: string) => Promise<void>
}) {
  const [now, setNow] = useState(() => Date.now())
  const [dragging, setDragging] = useState(false)
  const [disconnectOpen, setDisconnectOpen] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)
  const stickToBottom = useRef(true)

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [])

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return
    let disposed = false
    let unlisten: UnlistenFn | undefined

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          setDragging(false)
          if (event.payload.paths.length > 0) {
            void onSendFiles(event.payload.paths)
          }
        } else if (
          event.payload.type === "enter" ||
          event.payload.type === "over"
        ) {
          setDragging(true)
        } else {
          setDragging(false)
        }
      })
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [onSendFiles])

  useEffect(() => {
    const element = scrollRef.current
    if (!element) return
    const last = messages[messages.length - 1]
    if (stickToBottom.current || last?.direction === "out") {
      element.scrollTop = element.scrollHeight
    }
  }, [messages])

  const peer = session.peer
  const duration = formatDuration(session.startedAt, now)

  const handleScroll = () => {
    const element = scrollRef.current
    if (!element) return
    const distance =
      element.scrollHeight - element.scrollTop - element.clientHeight
    stickToBottom.current = distance < 96
  }

  return (
    <div className="relative flex h-full flex-col overflow-hidden">
      <header className="relative z-20 flex h-16 shrink-0 items-center justify-between border-b bg-background/85 px-5 backdrop-blur">
        <div className="flex min-w-0 items-center">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h1 className="truncate text-[13px] font-medium tracking-tight">
                {peer.name}
              </h1>
              <span className="flex shrink-0 items-center gap-1 text-[10px] font-medium text-emerald-600 dark:text-emerald-400">
                <span className="size-1.5 rounded-full bg-emerald-500" />
                Connected
              </span>
            </div>
            <p className="mt-0.5 truncate text-[10.5px] text-muted-foreground">
              {platformLabel(peer.platform)}
              <span className="tabular-nums"> · {peer.ip}</span>
              <span className="tabular-nums"> · Connected {duration}</span>
            </p>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <ModeToggle />
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="gap-1.5 text-muted-foreground hover:border-destructive/25 hover:bg-destructive/5 hover:text-destructive"
            onClick={() => setDisconnectOpen(true)}
          >
            <UnplugIcon />
            Disconnect
          </Button>
        </div>
      </header>

      <main className="flex min-h-0 flex-1 flex-col">
        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className="chat-scroll min-h-0 flex-1 overflow-y-auto overscroll-contain"
        >
          <div className="mx-auto flex min-h-full w-full max-w-3xl flex-col px-6 py-5">
            {messages.length === 0 ? (
              <div className="flex flex-1 flex-col items-center justify-center py-14 text-center">
                <div className="relative mb-5">
                  <div className="grid size-12 place-items-center rounded-2xl border bg-muted/40">
                    <LockKeyholeIcon className="size-5" strokeWidth={1.7} />
                  </div>
                  <span className="absolute -right-1 -bottom-1 grid size-4.5 place-items-center rounded-full bg-emerald-500 ring-4 ring-background">
                    <CheckIcon className="size-2.5 text-white" strokeWidth={3} />
                  </span>
                </div>
                <p className="text-[13px] font-medium">
                  Secure connection established
                </p>
                <p className="mt-1.5 max-w-[330px] text-[11px] leading-5 text-muted-foreground">
                  You can now send messages and files.
                  <br />
                  History clears when you disconnect.
                </p>
                <div className="mt-5 flex items-center gap-1.5 rounded-full border bg-muted/25 px-2.5 py-1 text-[10px] text-muted-foreground">
                  <ShieldCheckIcon className="size-3" />
                  No servers involved
                </div>
                <p className="mt-7 text-[10px] text-muted-foreground/70">
                  Received files are saved to Downloads/Localink
                </p>
              </div>
            ) : (
              <div className="mt-auto">
                {messages.map((message, index) => {
                  const previous = messages[index - 1]
                  const grouped =
                    Boolean(previous) &&
                    previous.direction === message.direction &&
                    message.at - previous.at < 5 * 60 * 1000
                  const transfer = message.transfer
                    ? transfers[message.transfer.id] ?? message.transfer
                    : null

                  return (
                    <ChatMessageRow
                      key={message.id}
                      message={message}
                      transfer={transfer}
                      grouped={grouped}
                      onCopy={(text) => void onCopyText(text)}
                      onPause={(id) => void onPauseTransfer(id)}
                      onResume={(id) => void onResumeTransfer(id)}
                      onCancel={(id) => void onCancelTransfer(id)}
                      onOpen={(path) => void onOpenPath(path)}
                      onReveal={(path) => void onRevealPath(path)}
                      onRetry={(path) => void onSendFiles([path])}
                    />
                  )
                })}
              </div>
            )}
          </div>
        </div>

        <ChatComposer
          disabled={false}
          onSendText={onSendText}
          onSendFiles={onSendFiles}
        />
      </main>

      {dragging && (
        <div className="pointer-events-none absolute inset-3 z-40 flex flex-col items-center justify-center rounded-2xl border-2 border-dashed border-primary/40 bg-background/88 backdrop-blur-sm">
          <div className="grid size-12 place-items-center rounded-2xl bg-primary/10 text-primary">
            <UploadCloudIcon className="size-6" strokeWidth={1.7} />
          </div>
          <p className="mt-3 text-[13px] font-medium">Drop to send</p>
          <p className="mt-1 text-[11px] text-muted-foreground">
            Any file type
          </p>
        </div>
      )}

      <DisconnectDialog
        open={disconnectOpen}
        onOpenChange={setDisconnectOpen}
        onConfirm={() => {
          setDisconnectOpen(false)
          void onDisconnect()
        }}
      />
    </div>
  )
}
