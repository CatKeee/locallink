import { useRef, useState } from "react"
import { open } from "@tauri-apps/plugin-dialog"
import { ArrowUpIcon, LockKeyholeIcon, PaperclipIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { toast } from "@/components/ui/toast"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

export function ChatComposer({
  disabled,
  onSendText,
  onSendFiles,
}: {
  disabled: boolean
  onSendText: (text: string) => Promise<boolean>
  onSendFiles: (paths: string[]) => Promise<void>
}) {
  const [text, setText] = useState("")
  const [sending, setSending] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const canSend = !disabled && !sending && text.trim().length > 0

  const submit = async () => {
    const value = text
    if (!value.trim() || sending || disabled) return
    setSending(true)
    const sent = await onSendText(value)
    setSending(false)
    if (sent) {
      setText("")
      textareaRef.current?.focus()
    }
  }

  const pickFiles = async () => {
    try {
      const selection = await open({
        multiple: true,
        title: "Choose files to send",
      })
      if (!selection) return
      const paths = Array.isArray(selection) ? selection : [selection]
      await onSendFiles(paths)
    } catch (error) {
      toast.add({
        type: "error",
        title: "Couldn't open the file picker",
        description: error instanceof Error ? error.message : String(error),
      })
    }
  }

  return (
    <div className="shrink-0 border-t bg-background/85 px-5 py-3.5 backdrop-blur">
      <div className="mx-auto w-full max-w-3xl">
        <div className="flex items-end gap-1.5 rounded-2xl border bg-card p-1.5 pl-2 shadow-xs transition-[border-color,box-shadow] focus-within:border-ring/60 focus-within:ring-3 focus-within:ring-ring/15">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  className="mb-0.5 shrink-0 text-muted-foreground hover:text-foreground"
                  onClick={() => void pickFiles()}
                  disabled={disabled}
                  aria-label="Choose files"
                />
              }
            >
              <PaperclipIcon />
            </TooltipTrigger>
            <TooltipContent>Send files</TooltipContent>
          </Tooltip>

          <Textarea
            ref={textareaRef}
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if (
                event.key === "Enter" &&
                !event.shiftKey &&
                !event.nativeEvent.isComposing
              ) {
                event.preventDefault()
                void submit()
              }
            }}
            rows={1}
            maxLength={12000}
            placeholder="Type a message, or drop files…"
            disabled={disabled}
            className="max-h-36 min-h-9 flex-1 resize-none border-0 bg-transparent px-1.5 py-1.5 text-[13px] leading-5 shadow-none focus-visible:ring-0 md:text-[13px] dark:bg-transparent"
          />

          <Button
            type="button"
            size="icon"
            className="mb-0.5 size-8 shrink-0 rounded-xl"
            disabled={!canSend}
            onClick={() => void submit()}
            aria-label="Send message"
          >
            <ArrowUpIcon />
          </Button>
        </div>

        <div className="mt-1.5 flex items-center justify-between px-1 text-[10.5px] text-muted-foreground">
          <span>Enter to send · Shift + Enter for a new line</span>
          <span className="flex items-center gap-1">
            <LockKeyholeIcon className="size-3" />
            Only in this session
          </span>
        </div>
      </div>
    </div>
  )
}
