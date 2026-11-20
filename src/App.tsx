import { useState } from "react"
import { Loader2Icon, RefreshCwIcon } from "lucide-react"

import { BrandLockup, BrandMark } from "@/components/brand"
import {
  IncomingRequestDialog,
  OutgoingRequestDialog,
} from "@/components/connection-dialogs"
import { RenameDeviceDialog } from "@/components/rename-device-dialog"
import { ThemeProvider } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useLocalink } from "@/hooks/use-localink"
import { ChatView } from "@/views/chat-view"
import { NearbyView } from "@/views/nearby-view"

function LoadingScreen() {
  return (
    <div className="flex h-svh flex-col items-center justify-center gap-4 bg-background">
      <BrandMark className="size-10 rounded-xl" />
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <Loader2Icon className="size-3.5 animate-spin" />
        Starting…
      </div>
    </div>
  )
}

function ErrorScreen({ message }: { message: string }) {
  return (
    <div className="relative flex h-svh flex-col items-center justify-center overflow-hidden bg-background px-6">
      <div
        className="pointer-events-none absolute inset-0 bg-[linear-gradient(to_right,var(--border)_1px,transparent_1px),linear-gradient(to_bottom,var(--border)_1px,transparent_1px)] bg-[size:56px_56px] opacity-40 [mask-image:radial-gradient(ellipse_55%_50%_at_50%_50%,black,transparent)]"
        aria-hidden="true"
      />
      <div className="relative w-full max-w-[420px] rounded-2xl border bg-card p-6 shadow-sm">
        <BrandLockup />
        <h1 className="mt-6 text-base font-medium">Localink couldn't start</h1>
        <p className="mt-2 text-xs leading-5 text-muted-foreground">
          If you're previewing this in a browser, start the desktop app:
        </p>
        <code className="mt-3 block rounded-lg border bg-muted/40 px-3 py-2 text-[11px]">
          pnpm tauri dev
        </code>
        <p className="mt-3 text-[10.5px] leading-4 break-words text-muted-foreground/80">
          {message}
        </p>
        <Button
          className="mt-5 w-full gap-1.5"
          onClick={() => window.location.reload()}
        >
          <RefreshCwIcon className="size-3.5" />
          Reload
        </Button>
      </div>
    </div>
  )
}

function LocalinkApp() {
  const localink = useLocalink()
  const [renameOpen, setRenameOpen] = useState(false)

  if (!localink.ready) return <LoadingScreen />
  if (localink.error || !localink.device) {
    return <ErrorScreen message={localink.error ?? "Device information unavailable"} />
  }

  return (
    <div className="h-svh overflow-hidden bg-background text-foreground">
      {localink.view === "nearby" && (
        <NearbyView
          device={localink.device}
          peers={localink.peers}
          network={localink.network}
          discoverable={localink.discoverable}
          version={localink.version}
          outgoingRequest={localink.outgoingRequest}
          onEditDevice={() => setRenameOpen(true)}
          onConnect={(peer) => void localink.connect(peer)}
          onDiscoverableChange={localink.setDiscoverable}
        />
      )}

      {localink.view === "chat" && localink.session && (
        <ChatView
          session={localink.session}
          messages={localink.messages}
          transfers={localink.transfers}
          onDisconnect={localink.disconnect}
          onSendText={localink.sendText}
          onSendFiles={localink.sendFiles}
          onPauseTransfer={localink.pauseTransfer}
          onResumeTransfer={localink.resumeTransfer}
          onCancelTransfer={localink.cancelTransfer}
          onOpenPath={localink.openPath}
          onRevealPath={localink.revealPath}
          onCopyText={localink.copyText}
        />
      )}

      <RenameDeviceDialog
        open={renameOpen}
        currentName={localink.device.name}
        onOpenChange={setRenameOpen}
        onSave={localink.renameDevice}
      />

      <IncomingRequestDialog
        request={localink.incomingRequest}
        onRespond={(accept) => void localink.respondIncoming(accept)}
      />

      <OutgoingRequestDialog
        request={localink.outgoingRequest}
        onCancel={() => void localink.cancelOutgoing()}
      />
    </div>
  )
}

function App() {
  return (
    <ThemeProvider defaultTheme="system" storageKey="localink-theme">
      <Toaster>
        <TooltipProvider delay={280}>
          <LocalinkApp />
        </TooltipProvider>
      </Toaster>
    </ThemeProvider>
  )
}

export default App
