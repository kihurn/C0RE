// Typed client for the admin API (/admin/api). Same origin only.

export type Reach = 'private' | 'public' | 'off'

export interface Limits {
  rate_per_min: number
  daily_cap: number
  max_concurrent: number
  max_ctx: number
  max_body_mb: number
}

export interface Key extends Limits {
  id: string
  project: string
  reach: Reach
  models: string[]
  note: string
  created_at: number
  last_used_at: number | null
  /** Whether the secret can be shown again (keys from before 0.2 can't). */
  revealable: boolean
}

export interface UsageRecord {
  ts: number
  key_id: string | null
  project: string | null
  entrance: 'private' | 'public'
  path: string
  model: string | null
  status: number
  block_reason: string | null
  latency_ms: number
  ttfb_ms: number | null
  prompt_tokens: number | null
  output_tokens: number | null
}

export interface HourUsage {
  hour: number
  requests: number
  blocked: number
}

export interface KeyUsage {
  key_id: string
  project: string
  requests: number
  blocked: number
  avg_latency_ms: number
  prompt_tokens: number
  output_tokens: number
}

export interface Summary {
  total: number
  blocked: number
  by_reason: [string, number][]
  per_key: KeyUsage[]
  per_hour: HourUsage[]
}

export interface LoadedModel {
  name: string
  size: number
  size_vram: number
  gpu_percent: number
  context: number | null
}

export interface Overview {
  engine: { online: boolean; version: string | null; loaded: LoadedModel[] }
  today: { requests: number; blocked: number; projects: number }
  per_hour: HourUsage[]
  recent: UsageRecord[]
  keys: number
  entrances: { private: { requests: number; blocked: number }; public: { requests: number; blocked: number } }
}

export interface Model {
  name: string
  size: number | null
  modified_at: string | null
  loaded: boolean
  vision: boolean
  thinking: boolean
  capabilities: string[]
}

export interface Settings {
  private_defaults: Limits
  public_defaults: Limits
  retention_days: number
  max_db_mb: number
}

export type PublicState =
  | { state: 'on'; url: string }
  | { state: 'off' }
  | { state: 'unavailable'; reason: string }

export interface NewKey {
  project: string
  reach: Reach
  models: string[]
  limits?: Limits
  note?: string
}

export type KeyPatch = Partial<Omit<Key, 'id' | 'created_at' | 'last_used_at' | 'revealable'>>

export type Result<T> = { ok: true; data: T } | { ok: false; status: number; code: string; message: string }

let csrf: string | null = null
export function setCsrf(token: string | null): void {
  csrf = token
}

async function call<T>(method: string, path: string, body?: unknown): Promise<Result<T>> {
  const headers: Record<string, string> = { accept: 'application/json' }
  if (body !== undefined) headers['content-type'] = 'application/json'
  if (method !== 'GET' && csrf) headers['x-c0re-csrf'] = csrf
  let res: Response
  try {
    res = await fetch(`/admin/api${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: 'same-origin'
    })
  } catch {
    return { ok: false, status: 0, code: 'network', message: "Can't reach C0RE. Check that the PC is on and Tailscale is connected." }
  }
  if (res.status === 204) return { ok: true, data: undefined as T }
  let json: unknown = null
  try {
    json = await res.json()
  } catch {
    /* empty or not JSON */
  }
  if (res.ok) return { ok: true, data: json as T }
  const err = (json as { error?: { code?: string; message?: string } } | null)?.error
  if (res.status === 401 && err?.code !== 'wrong_password') {
    return { ok: false, status: 401, code: 'unauthorized', message: err?.message ?? 'Please log in.' }
  }
  return {
    ok: false,
    status: res.status,
    code: err?.code ?? 'error',
    message: err?.message ?? `Request failed (${res.status}).`
  }
}

export const api = {
  async status() {
    const r = await call<{ password_set: boolean; logged_in: boolean; csrf: string | null }>('GET', '/status')
    if (r.ok && r.data.csrf) setCsrf(r.data.csrf)
    return r
  },
  async login(password: string) {
    const r = await call<{ csrf: string }>('POST', '/login', { password })
    if (r.ok) setCsrf(r.data.csrf)
    return r
  },
  async logout() {
    const r = await call<void>('POST', '/logout')
    setCsrf(null)
    return r
  },
  overview: () => call<Overview>('GET', '/overview'),
  keys: () => call<Key[]>('GET', '/keys'),
  createKey: (k: NewKey) => call<{ key: Key; secret: string }>('POST', '/keys', k),
  updateKey: (id: string, p: KeyPatch) => call<Key>('PATCH', `/keys/${encodeURIComponent(id)}`, p),
  revealKey: (id: string) => call<{ secret: string }>('POST', `/keys/${encodeURIComponent(id)}/reveal`),
  deleteKey: (id: string) => call<void>('DELETE', `/keys/${encodeURIComponent(id)}`),
  usage: (days: number) => call<{ days: number; summary: Summary }>('GET', `/usage?days=${days}`),
  models: () => call<Model[]>('GET', '/models'),
  settings: () => call<Settings>('GET', '/settings'),
  saveSettings: (s: Settings) => call<Settings>('PUT', '/settings', s),
  publicState: () => call<PublicState>('GET', '/public'),
  setPublic: (on: boolean) => call<PublicState>('POST', '/public', { on }),
  changePassword: (current: string, next: string) => call<void>('POST', '/password', { current, new: next })
}
