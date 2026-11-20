export function formatBytes(bytes: number, decimals = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KB", "MB", "GB", "TB"]
  const index = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1
  )
  const value = bytes / Math.pow(1024, index)
  const digits = index === 0 ? 0 : decimals
  return `${value.toFixed(digits).replace(/\.0$/, "")} ${units[index]}`
}

export function formatSpeed(bytesPerSecond: number): string {
  if (!bytesPerSecond || bytesPerSecond <= 0) return "0 B/s"
  return `${formatBytes(bytesPerSecond)}/s`
}

export function formatEta(seconds: number | null | undefined): string {
  if (!seconds || seconds <= 0 || !Number.isFinite(seconds)) return "Calculating"
  if (seconds < 60) return `${Math.max(1, Math.ceil(seconds))}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  return rest > 0 ? `${hours}h ${rest}m` : `${hours}h`
}

export function formatTime(timestamp: number): string {
  return new Intl.DateTimeFormat("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(timestamp))
}

export function formatDuration(startedAt: number, now: number): string {
  const total = Math.max(0, Math.floor((now - startedAt) / 1000))
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = total % 60
  if (hours > 0) {
    return [hours, minutes, seconds]
      .map((part) => String(part).padStart(2, "0"))
      .join(":")
  }
  return [minutes, seconds]
    .map((part) => String(part).padStart(2, "0"))
    .join(":")
}

export function platformLabel(platform?: string): string {
  switch (platform) {
    case "macos":
      return "macOS"
    case "windows":
      return "Windows"
    case "linux":
      return "Linux"
    default:
      return "Unknown"
  }
}

export function initialOf(name?: string): string {
  const trimmed = (name ?? "").trim()
  if (!trimmed) return "L"
  const chars = Array.from(trimmed)
  return chars[0]!.toUpperCase()
}

export function extensionOf(name: string): string {
  const index = name.lastIndexOf(".")
  if (index <= 0 || index === name.length - 1) return ""
  return name.slice(index + 1).toLowerCase()
}

export function isImageFile(name: string): boolean {
  return ["png", "jpg", "jpeg", "gif", "webp", "heic", "avif", "svg"].includes(
    extensionOf(name)
  )
}

export function truncateMiddle(value: string, max = 40): string {
  if (value.length <= max) return value
  const side = Math.floor((max - 1) / 2)
  return `${value.slice(0, side)}…${value.slice(-side)}`
}
