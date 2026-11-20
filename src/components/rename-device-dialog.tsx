import { useEffect, useState } from "react"
import { Loader2Icon, MonitorSmartphoneIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

export function RenameDeviceDialog({
  open,
  currentName,
  onOpenChange,
  onSave,
}: {
  open: boolean
  currentName: string
  onOpenChange: (open: boolean) => void
  onSave: (name: string) => Promise<boolean>
}) {
  const [name, setName] = useState(currentName)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    if (open) {
      setName(currentName)
      setSaving(false)
    }
  }, [open, currentName])

  const trimmed = name.trim()
  const canSave = trimmed.length > 0 && trimmed !== currentName && !saving

  const submit = async () => {
    if (!canSave) return
    setSaving(true)
    const saved = await onSave(trimmed)
    setSaving(false)
    if (saved) onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[400px]">
        <form
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}
        >
          <DialogHeader>
            <div className="mb-1 grid size-9 place-items-center rounded-xl border bg-muted/50">
              <MonitorSmartphoneIcon className="size-4" strokeWidth={1.8} />
            </div>
            <DialogTitle>Name this device</DialogTitle>
            <DialogDescription>
              This name is visible to other devices nearby.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-2">
            <Label htmlFor="device-name">Device name</Label>
            <Input
              id="device-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              maxLength={32}
              placeholder="e.g. Alex's MacBook Pro"
              autoFocus
              className="h-9"
            />
            <p className="text-[11px] text-muted-foreground">
              Up to 32 characters. You can change it anytime.
            </p>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={!canSave}>
              {saving && <Loader2Icon className="animate-spin" />}
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
