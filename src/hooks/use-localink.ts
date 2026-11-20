import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { toast } from "@/components/ui/toast"
import type {
  Bootstrap,
  ChatMessage,
  DeviceInfo,
  IncomingRequest,
  NetworkInfo,
  OutgoingRequest,
  Peer,
  SessionEnded,
  SessionInfo,
  Transfer,
  View,
} from "@/types"

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error
  if (error instanceof Error) return error.message
  return "Something went wrong"
}

export function useLocalink() {
  const [device, setDevice] = useState<DeviceInfo | null>(null)
  const [peers, setPeers] = useState<Peer[]>([])
  const [network, setNetwork] = useState<NetworkInfo | null>(null)
  const [session, setSession] = useState<SessionInfo | null>(null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [transfers, setTransfers] = useState<Record<string, Transfer>>({})
  const [incomingRequest, setIncomingRequest] =
    useState<IncomingRequest | null>(null)
  const [outgoingRequest, setOutgoingRequest] =
    useState<OutgoingRequest | null>(null)
  const [view, setView] = useState<View>("nearby")
  const [discoverable, setDiscoverableState] = useState(true)
  const [downloadDir, setDownloadDir] = useState("")
  const [version, setVersion] = useState("")
  const [ready, setReady] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const sessionRef = useRef<SessionInfo | null>(null)
  const outgoingRef = useRef<OutgoingRequest | null>(null)

  useEffect(() => {
    sessionRef.current = session
  }, [session])

  useEffect(() => {
    outgoingRef.current = outgoingRequest
  }, [outgoingRequest])

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) {
      setError("Start the desktop app with pnpm tauri dev")
      setReady(true)
      return
    }

    let disposed = false
    const subscriptions: Promise<UnlistenFn>[] = []
    const addListener = <T,>(event: string, handler: (payload: T) => void) => {
      subscriptions.push(
        listen<T>(event, (event) => {
          if (!disposed) handler(event.payload)
        })
      )
    }

    addListener<Peer[]>("peers-updated", setPeers)

    addListener<NetworkInfo>("network-updated", setNetwork)

    addListener<IncomingRequest>("incoming-request", (request) => {
      if (sessionRef.current) {
        void invoke("respond_connection", {
          requestId: request.requestId,
          accept: false,
        }).catch(() => undefined)
        return
      }
      setIncomingRequest(request)
    })

    addListener<{ requestId: string }>("incoming-request-closed", (event) => {
      setIncomingRequest((current) =>
        current?.requestId === event.requestId ? null : current
      )
    })

    addListener<OutgoingRequest>("outgoing-request", (request) => {
      setOutgoingRequest(request)
    })

    addListener<OutgoingRequest>("outgoing-request-updated", (request) => {
      setOutgoingRequest((current) =>
        current?.requestId === request.requestId ? null : current
      )

      if (request.status === "rejected") {
        toast.add({
          type: "error",
          title: "Connection declined",
          description: request.reason ?? "Try again in a moment.",
        })
      } else if (request.status === "failed" || request.status === "timeout") {
        toast.add({
          type: "error",
          title:
            request.status === "timeout"
              ? "Connection timed out"
              : "Connection failed",
          description:
            request.reason ?? "Make sure both devices are still on the same network.",
        })
      }
    })

    addListener<SessionInfo>("session-started", (started) => {
      setSession(started)
      setMessages([])
      setTransfers({})
      setIncomingRequest(null)
      setOutgoingRequest(null)
      setView("chat")
    })

    addListener<SessionEnded>("session-ended", (ended) => {
      if (!sessionRef.current) return
      setSession(null)
      setMessages([])
      setTransfers({})
      setIncomingRequest(null)
      setOutgoingRequest(null)
      setView("nearby")

      if (ended.reason === "local") {
        toast.add({
          type: "info",
          title: "Disconnected",
          description: "Messages and transfers from this session were cleared.",
        })
      } else if (ended.reason === "peer") {
        toast.add({
          type: "info",
          title: "The other device disconnected",
          description: "This session's history was cleared.",
        })
      } else {
        toast.add({
          type: "error",
          title: "Connection interrupted",
          description: ended.message ?? "The network connection was lost.",
        })
      }
    })

    addListener<ChatMessage>("message-added", (message) => {
      setMessages((current) => {
        const next = [...current, message]
        return next.length > 800 ? next.slice(next.length - 800) : next
      })
      if (message.transfer) {
        const transfer = message.transfer
        setTransfers((current) => ({ ...current, [transfer.id]: transfer }))
      }
    })

    addListener<Transfer>("transfer-updated", (transfer) => {
      setTransfers((current) => ({ ...current, [transfer.id]: transfer }))
    })

    const initialize = async () => {
      try {
        const unlisteners = await Promise.all(subscriptions)
        if (disposed) {
          unlisteners.forEach((unlisten) => unlisten())
          return
        }

        const bootstrap = await invoke<Bootstrap>("get_bootstrap")
        if (disposed) return
        setDevice(bootstrap.device)
        setPeers(bootstrap.peers)
        setNetwork(bootstrap.network)
        setDiscoverableState(bootstrap.discoverable)
        setDownloadDir(bootstrap.downloadDir)
        setVersion(bootstrap.version)
        if (bootstrap.session) {
          setSession(bootstrap.session)
          setView("chat")
        }
        setReady(true)
      } catch (caught) {
        if (!disposed) {
          setError(errorMessage(caught))
          setReady(true)
        }
      }
    }

    void initialize()

    return () => {
      disposed = true
      subscriptions.forEach((subscription) => {
        void subscription.then((unlisten) => unlisten()).catch(() => undefined)
      })
    }
  }, [])

  const renameDevice = useCallback(async (name: string) => {
    try {
      const next = await invoke<DeviceInfo>("rename_device", { name })
      setDevice(next)
      toast.add({
        type: "success",
        title: "Device name updated",
        description: "Nearby devices will see the new name shortly.",
      })
      return true
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't rename device",
        description: errorMessage(caught),
      })
      return false
    }
  }, [])

  const setDiscoverable = useCallback(async (next: boolean) => {
    try {
      const value = await invoke<boolean>("set_discoverable", {
        discoverable: next,
      })
      setDiscoverableState(value)
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't change visibility",
        description: errorMessage(caught),
      })
    }
  }, [])

  const connect = useCallback(async (peer: Peer) => {
    try {
      await invoke("connect_to_peer", { peerId: peer.id })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't start connection",
        description: errorMessage(caught),
      })
    }
  }, [])

  const cancelOutgoing = useCallback(async () => {
    const request = outgoingRef.current
    if (!request) return
    setOutgoingRequest(null)
    try {
      await invoke("cancel_outgoing", { requestId: request.requestId })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't cancel the request",
        description: errorMessage(caught),
      })
    }
  }, [])

  const respondIncoming = useCallback(async (accept: boolean) => {
    const request = incomingRequest
    if (!request) return
    setIncomingRequest(null)
    try {
      await invoke("respond_connection", {
        requestId: request.requestId,
        accept,
      })
    } catch (caught) {
      toast.add({
        type: "error",
        title: accept ? "Couldn't connect" : "Action failed",
        description: errorMessage(caught),
      })
    }
  }, [incomingRequest])

  const sendText = useCallback(async (text: string) => {
    if (!sessionRef.current) return false
    try {
      await invoke("send_text", {
        sessionId: sessionRef.current.id,
        text,
      })
      return true
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Message failed",
        description: errorMessage(caught),
      })
      return false
    }
  }, [])

  const sendFiles = useCallback(async (paths: string[]) => {
    if (!sessionRef.current || paths.length === 0) return
    try {
      await invoke("send_files", {
        sessionId: sessionRef.current.id,
        paths,
      })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "File transfer failed",
        description: errorMessage(caught),
      })
    }
  }, [])

  const pauseTransfer = useCallback(async (transferId: string) => {
    try {
      await invoke("pause_transfer", { transferId })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't pause the transfer",
        description: errorMessage(caught),
      })
    }
  }, [])

  const resumeTransfer = useCallback(async (transferId: string) => {
    try {
      await invoke("resume_transfer", { transferId })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't resume the transfer",
        description: errorMessage(caught),
      })
    }
  }, [])

  const cancelTransfer = useCallback(async (transferId: string) => {
    try {
      await invoke("cancel_transfer", { transferId })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't cancel the transfer",
        description: errorMessage(caught),
      })
    }
  }, [])

  const disconnect = useCallback(async () => {
    try {
      await invoke("disconnect_session")
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't disconnect",
        description: errorMessage(caught),
      })
    }
  }, [])

  const openPath = useCallback(async (path: string) => {
    try {
      await invoke("open_path", { path })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't open file",
        description: errorMessage(caught),
      })
    }
  }, [])

  const revealPath = useCallback(async (path: string) => {
    try {
      await invoke("reveal_path", { path })
    } catch (caught) {
      toast.add({
        type: "error",
        title: "Couldn't show file",
        description: errorMessage(caught),
      })
    }
  }, [])

  const copyText = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
    } catch {
      const textarea = document.createElement("textarea")
      textarea.value = text
      textarea.style.position = "fixed"
      textarea.style.opacity = "0"
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand("copy")
      textarea.remove()
    }
    toast.add({ type: "success", title: "Copied to clipboard" })
  }, [])

  return {
    device,
    peers,
    network,
    session,
    messages,
    transfers,
    incomingRequest,
    outgoingRequest,
    view,
    discoverable,
    downloadDir,
    version,
    ready,
    error,
    renameDevice,
    setDiscoverable,
    connect,
    cancelOutgoing,
    respondIncoming,
    sendText,
    sendFiles,
    pauseTransfer,
    resumeTransfer,
    cancelTransfer,
    disconnect,
    openPath,
    revealPath,
    copyText,
  }
}

export type Localink = ReturnType<typeof useLocalink>
