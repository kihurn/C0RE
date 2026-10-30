export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`
  if (ms < 10_000) return `${(ms / 1000).toFixed(1)} s`
  if (ms < 60_000) return `${Math.round(ms / 1000)} s`
  return `${Math.round(ms / 60_000)} min`
}

const trim = (n: number) => (n >= 10 ? String(Math.round(n)) : n.toFixed(1).replace(/\.0$/, ''))

/** Bytes below 1 MB use 1024 steps (as file sizes are usually shown); larger use 1000 (as Ollama does). */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1_000_000) return `${trim(n / 1024)} KB`
  const units = ['MB', 'GB', 'TB']
  let v = n / 1_000_000
  let i = 0
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000
    i++
  }
  return `${trim(v)} ${units[i]}`
}

export function formatCount(n: number): string {
  if (n < 10_000) return n.toLocaleString('en-US')
  if (n < 1_000_000) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k`
  return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`
}

/** `ts` and `now` are Unix seconds. */
export function relativeTime(ts: number | null | undefined, now = Math.floor(Date.now() / 1000)): string {
  if (ts == null) return 'never'
  const d = Math.max(0, now - ts)
  if (d < 60) return 'just now'
  if (d < 3600) return `${Math.floor(d / 60)} min ago`
  if (d < 86_400) return `${Math.floor(d / 3600)} h ago`
  return `${Math.floor(d / 86_400)} d ago`
}

export function clockTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
