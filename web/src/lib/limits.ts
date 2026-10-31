// Slider ranges for key limits and history settings, shared by Settings and the key editor.
import type { Limits } from './api'

export const limitSliders: { key: keyof Limits; label: string; unit?: string; min?: number; max?: number; stops?: number[] }[] = [
  { key: 'rate_per_min', label: 'Per minute', min: 1, max: 300 },
  { key: 'daily_cap', label: 'Per day', stops: [50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000] },
  { key: 'max_concurrent', label: 'At once', min: 1, max: 8 },
  { key: 'max_ctx', label: 'Max context', unit: 'tokens', stops: [2048, 4096, 8192, 16384, 32768] },
  { key: 'max_body_mb', label: 'Max request', unit: 'MB', stops: [1, 2, 5, 8, 10, 20, 50, 100] }
]

export const retentionStops = [7, 14, 30, 60, 90, 180, 365]
export const dbSizeStops = [25, 50, 100, 250, 500, 1000]
