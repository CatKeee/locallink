import {
  Clock3Icon,
  InboxIcon,
  Loader2Icon,
  ShieldCheckIcon,
  UnplugIcon,
} from "lucide-react"

import { DeviceAvatar } from "@/components/device-avatar"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { platformLabel } from "@/lib/format"
import type { IncomingRequest, OutgoingRequest } from "@/types"

export function IncomingRequestDialog({
  request,
  onRespond,
}: {
  request: IncomingRequest | null
  onRespond: (accept: boolean) => void
}) {
  if (!request) return null

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onRespond(false)
      }}
      disablePointerDismissal
    >
      <DialogContent showCloseButton={false} className="gap-0 p-0 sm:max-w-[400px]">
        <div className="p-5">
          <DialogHeader>
            <div className="mb-1 flex items-center gap-2">
              <span className="grid size-9 place-items-center rounded-xl border bg-muted/50">
                <InboxIcon className="size-4" strokeWidth={1.8} />
              </span>
            </div>
            <DialogTitle>Connection request</DialogTitle>
            <DialogDescription>
              Another device wants to connect to this one.
            </DialogDescription>
          </DialogHeader>

          <div className="mt-4 flex items-center gap-3 rounded-xl border bg-muted/25 p-3">
            <DeviceAvatar
              name={request.peer.name}
              size="default"
              online
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-medium">
                {request.peer.name}
              </p>
              <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                {platformLabel(request.peer.platform)} · {request.peer.ip}
              </p>
            </div>
          </div>

          <p className="mt-4 flex items-start gap-2 text-[11px] leading-5 text-muted-foreground">
            <ShieldCheckIcon className="mt-0.5 size-3.5 shrink-0" />
            Once approved, you can exchange messages and files. Session data is kept only while connected.
          </p>
        </div>

        <DialogFooter className="mx-0 mb-0 rounded-b-xl border-t bg-muted/25 p-3">
          <Button variant="outline" onClick={() => onRespond(false)}>
            Decline
          </Button>
          <Button onClick={() => onRespond(true)}>Allow</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function OutgoingRequestDialog({
  request,
  onCancel,
}: {
  request: OutgoingRequest | null
  onCancel: () => void
}) {
  if (!request) return null

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onCancel()
      }}
      disablePointerDismissal
    >
      <DialogContent showCloseButton={false} className="gap-0 p-0 sm:max-w-[380px]">
        <div className="flex flex-col items-center p-6 pb-5 text-center">
          <div className="relative">
            <DeviceAvatar name={request.peer.name} size="lg" />
            <span className="absolute -right-1 -bottom-1 grid size-5 place-items-center rounded-full bg-background text-primary ring-1 ring-primary/25">
              <Loader2Icon className="size-3 animate-spin" />
            </span>
          </div>
          <DialogTitle className="mt-4">Waiting for approval</DialogTitle>
          <DialogDescription className="mt-1.5 max-w-[290px]">
            {request.peer.name} will see your request in Localink.
          </DialogDescription>
          <div className="mt-5 flex w-full items-center justify-center gap-2 rounded-lg border bg-muted/25 px-3 py-2 text-[11px] text-muted-foreground">
            <Clock3Icon className="size-3.5" />
            This request expires in 45 seconds
          </div>
        </div>
        <DialogFooter className="mx-0 mb-0 rounded-b-xl border-t bg-muted/25 p-3">
          <Button variant="outline" className="w-full" onClick={onCancel}>
            Cancel
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function DisconnectDialog({
  open,
  onOpenChange,
  onConfirm,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[400px]">
        <DialogHeader>
          <div className="mb-1 grid size-9 place-items-center rounded-xl border border-destructive/20 bg-destructive/10 text-destructive">
            <UnplugIcon className="size-4" strokeWidth={1.8} />
          </div>
          <DialogTitle>Disconnect this session?</DialogTitle>
          <DialogDescription>
            Messages and unfinished transfers will be cleared. Files you have already received stay in your Downloads folder.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={onConfirm}>
            Disconnect
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
