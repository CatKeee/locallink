import { useState, type ReactNode } from "react"
import type { LucideIcon } from "lucide-react"
import {
  ArrowUpRightIcon,
  EyeOffIcon,
  LaptopIcon,
  PencilIcon,
  ShieldCheckIcon,
  TriangleAlertIcon,
  WifiIcon,
} from "lucide-react"

import { BrandLockup } from "@/components/brand"
import { DeviceAvatar } from "@/components/device-avatar"
import { ModeToggle } from "@/components/mode-toggle"
import { Button } from "@/components/ui/button"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Switch } from "@/components/ui/switch"
import { platformLabel } from "@/lib/format"
import { cn } from "@/lib/utils"
import type { DeviceInfo, NetworkInfo, OutgoingRequest, Peer } from "@/types"

function StatusDot({ tone }: { tone: "online" | "idle" | "warning" }) {
  return (
    <span className="relative flex size-1.5 shrink-0">
      {tone === "online" && (
        <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-60" />
      )}
      <span
        className={cn(
          "relative inline-flex size-1.5 rounded-full",
          tone === "online" && "bg-emerald-500",
          tone === "idle" && "bg-muted-foreground/40",
          tone === "warning" && "bg-amber-500"
        )}
      />
    </span>
  )
}

function Chip({
  icon: Icon,
  children,
}: {
  icon: LucideIcon
  children: ReactNode
}) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border bg-background px-2.5 py-1 text-[10px] text-muted-foreground">
      <Icon className="size-3" strokeWidth={1.7} />
      {children}
    </span>
  )
}

function RadarVisual() {
  return (
    <div
      className="relative grid size-32 shrink-0 place-items-center"
      aria-hidden="true"
    >
      <span className="absolute inset-0 rounded-full bg-emerald-500/[0.035] dark:bg-emerald-400/[0.05]" />

      <div className="absolute inset-0 overflow-hidden rounded-full">
        <div className="absolute inset-0 animate-[spin_4s_linear_infinite]">
          <div className="absolute inset-0 rounded-full bg-[conic-gradient(from_0deg,rgba(16,185,129,0.28),rgba(16,185,129,0)_82deg)]" />
          <span className="absolute top-0 left-1/2 h-1/2 w-px -translate-x-1/2 bg-gradient-to-t from-emerald-500/0 to-emerald-500/80" />
        </div>
      </div>

      <span className="absolute inset-0 rounded-full border border-border/70" />
      <span className="absolute inset-[15%] rounded-full border border-border/60" />
      <span className="absolute inset-[30%] rounded-full border border-border/50" />
      <span className="absolute inset-x-0 top-1/2 h-px bg-border/50" />
      <span className="absolute inset-y-0 left-1/2 w-px bg-border/50" />

      <span className="absolute top-[30%] left-[68%] size-1 rounded-full bg-emerald-500/70 shadow-[0_0_0_3px_rgba(16,185,129,0.08)]" />
      <span className="absolute top-[66%] left-[34%] size-0.5 rounded-full bg-emerald-500/50" />
      <span className="relative size-1.5 rounded-full bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]" />
    </div>
  )
}

function EmptyState({
  discoverable,
  network,
}: {
  discoverable: boolean
  network: NetworkInfo | null
}) {
  const available = Boolean(network?.available)
  const hasNetwork = Boolean(network?.ip) && network?.ip !== "0.0.0.0"
  const searching = available && hasNetwork && discoverable

  const title = !available
    ? "Local discovery didn't start"
    : !hasNetwork
      ? "Not connected to a network"
      : !discoverable
        ? "You're hidden"
        : "Looking for nearby devices"

  const description = !available
    ? "Check the firewall, or quit another Localink instance and reopen the app."
    : !hasNetwork
      ? "Connect to Wi-Fi or Ethernet and Localink will search again automatically."
      : !discoverable
        ? "Nearby devices can't see you. Turn on Discoverable to let them find this device."
        : "Open Localink on another computer and keep it on the same network."

  return (
    <div className="flex min-h-[292px] items-center justify-center rounded-2xl border bg-muted/10 p-8">
      <div className="flex w-full max-w-[560px] flex-col items-center gap-7 md:flex-row md:gap-9">
        {searching ? (
          <RadarVisual />
        ) : (
          <div
            className="relative grid size-32 shrink-0 place-items-center"
            aria-hidden="true"
          >
            <span className="absolute inset-0 rounded-full border border-border/50" />
            <span className="absolute inset-[20%] rounded-full border border-border/40" />
            <span className="relative grid size-12 place-items-center rounded-2xl border bg-background shadow-xs">
              {!discoverable && available && hasNetwork ? (
                <EyeOffIcon
                  className="size-5 text-muted-foreground"
                  strokeWidth={1.6}
                />
              ) : (
                <TriangleAlertIcon
                  className="size-5 text-amber-500"
                  strokeWidth={1.6}
                />
              )}
            </span>
          </div>
        )}

        <div className="min-w-0 flex-1 text-center md:text-left">
          <h2 className="text-[15px] font-medium">{title}</h2>
          <p className="mt-1.5 max-w-[420px] text-[11px] leading-5 text-muted-foreground">
            {description}
          </p>

          {searching && (
            <>
              <div className="mt-4 flex flex-wrap items-center justify-center gap-2 md:justify-start">
                <Chip icon={WifiIcon}>Same Wi-Fi</Chip>
                <Chip icon={LaptopIcon}>Localink open</Chip>
                <Chip icon={ShieldCheckIcon}>Local network</Chip>
              </div>
              <p className="mt-3 text-[10px] text-muted-foreground/70">
                Can't find it? Check the firewall and the router's AP isolation.
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  )
}

export function NearbyView({
  device,
  peers,
  network,
  discoverable,
  version,
  outgoingRequest,
  onEditDevice,
  onConnect,
  onDiscoverableChange,
}: {
  device: DeviceInfo
  peers: Peer[]
  network: NetworkInfo | null
  discoverable: boolean
  version: string
  outgoingRequest: OutgoingRequest | null
  onEditDevice: () => void
  onConnect: (peer: Peer) => void
  onDiscoverableChange: (next: boolean) => Promise<void>
}) {
  const [savingVisibility, setSavingVisibility] = useState(false)
  const available = Boolean(network?.available)
  const hasNetwork = Boolean(network?.ip) && network?.ip !== "0.0.0.0"
  const sortedPeers = [...peers].sort((left, right) => {
    if (left.busy !== right.busy) return left.busy ? 1 : -1
    return left.name.localeCompare(right.name, "en")
  })

  const statusTone: "online" | "idle" | "warning" =
    !available || !hasNetwork
      ? "warning"
      : discoverable
        ? "online"
        : "idle"

  const statusText = !available
    ? "Discovery unavailable"
    : !hasNetwork
      ? "Offline"
      : discoverable
        ? "Visible"
        : "Hidden"

  const handleVisibility = async (next: boolean) => {
    if (savingVisibility) return
    setSavingVisibility(true)
    await onDiscoverableChange(next)
    setSavingVisibility(false)
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-background">
      <header className="shrink-0">
        <div className="mx-auto flex h-16 w-full max-w-[760px] items-center justify-between px-6">
          <BrandLockup />
          <ModeToggle />
        </div>
      </header>

      <ScrollArea className="min-h-0 flex-1">
        <div className="mx-auto w-full max-w-[760px] px-6 pt-6 pb-10">
          <h1 className="sr-only">Nearby devices</h1>
          <section className="flex items-center gap-3 rounded-2xl border bg-card p-3.5 shadow-xs">
            <DeviceAvatar name={device.name} self size="lg" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <p className="truncate text-[13px] font-medium">
                  {device.name}
                </p>
                <button
                  type="button"
                  onClick={onEditDevice}
                  className="grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground/60 transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/40"
                  aria-label="Rename device"
                >
                  <PencilIcon className="size-3" />
                </button>
              </div>
              <p className="mt-0.5 truncate text-[10.5px] text-muted-foreground">
                {platformLabel(device.platform)}
                {hasNetwork && (
                  <span className="tabular-nums"> · {network?.ip}</span>
                )}
              </p>
            </div>

            <div className="h-9 w-px shrink-0 bg-border" />

            <div className="flex shrink-0 items-center gap-3 pl-1">
              <div className="text-right">
                <p className="text-[11px] font-medium">Discoverable</p>
                <p className="mt-0.5 flex items-center justify-end gap-1.5 text-[9.5px] text-muted-foreground">
                  <StatusDot tone={statusTone} />
                  {statusText}
                </p>
              </div>
              <Switch
                checked={discoverable}
                onCheckedChange={(checked) => void handleVisibility(checked)}
                disabled={!available || savingVisibility}
                aria-label="Discoverable"
              />
            </div>
          </section>

          <p className="mt-2.5 flex items-start gap-1.5 text-[10px] leading-4 text-muted-foreground">
            <ShieldCheckIcon className="mt-0.5 size-3 shrink-0" />
            {discoverable
              ? "Visible on your local network while the app is open. You can still connect to other devices when hidden."
              : "Nearby devices can't see you. You can still connect to devices below."}
          </p>

          <div className="mt-8">
            {sortedPeers.length === 0 ? (
              <EmptyState discoverable={discoverable} network={network} />
            ) : (
              <>
                <div className="mb-2 flex items-center justify-between">
                  <h2 className="text-[11px] font-medium tracking-[0.08em] text-muted-foreground">
                    Available devices
                  </h2>
                  <span className="text-[10px] text-muted-foreground">
                    Connections require approval
                  </span>
                </div>

                <ScrollArea
                  className="rounded-xl border bg-card shadow-xs"
                  style={{
                    height: Math.min(sortedPeers.length, 4) * 64 + 2,
                  }}
                >
                  <div>
                    {sortedPeers.map((peer) => (
                      <div
                        key={peer.id}
                        className="group flex h-16 items-center gap-3.5 border-b border-border/70 px-4 transition-colors last:border-b-0 hover:bg-muted/25"
                      >
                        <DeviceAvatar name={peer.name} online={!peer.busy} />
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-[12.5px] font-medium">
                            {peer.name}
                          </p>
                          <p className="mt-0.5 truncate text-[10.5px] text-muted-foreground">
                            {platformLabel(peer.platform)}
                            <span className="tabular-nums"> · {peer.ip}</span>
                          </p>
                        </div>
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          disabled={peer.busy || Boolean(outgoingRequest)}
                          onClick={() => onConnect(peer)}
                          className="shrink-0"
                        >
                          {peer.busy ? "In session" : "Connect"}
                          {!peer.busy && <ArrowUpRightIcon />}
                        </Button>
                      </div>
                    ))}
                  </div>
                </ScrollArea>
              </>
            )}
          </div>
        </div>
      </ScrollArea>

      <footer className="shrink-0 border-t">
        <div className="mx-auto flex h-10 w-full max-w-[760px] items-center justify-between gap-4 px-6 text-[10px] text-muted-foreground">
          <span className="truncate">
            Connections require approval · History clears on disconnect
          </span>
          <span className="flex shrink-0 items-center gap-2">
            {version && <span>v{version}</span>}
            <span className="text-muted-foreground/40">·</span>
            <span className="flex items-center gap-1">
              <ShieldCheckIcon className="size-3" />
              Peer-to-peer
            </span>
          </span>
        </div>
      </footer>
    </div>
  )
}
