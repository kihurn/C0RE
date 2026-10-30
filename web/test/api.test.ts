import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api, setCsrf } from '../src/lib/api'

type Call = { url: string; init: RequestInit }
let calls: Call[] = []

function mockFetch(status: number, body: unknown) {
  calls = []
  globalThis.fetch = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} })
    return new Response(status === 204 ? null : JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' }
    })
  }) as typeof fetch
}

const header = (c: Call, name: string) => new Headers(c.init.headers).get(name)

beforeEach(() => setCsrf(null))

describe('api', () => {
  it('sends the csrf header on POST, PATCH and DELETE but not GET', async () => {
    setCsrf('tok123')
    mockFetch(200, { key: {}, secret: 'c0re_x' })
    await api.createKey({ project: 'lenz', reach: 'private', models: [] })
    expect(calls[0]!.url).toBe('/admin/api/keys')
    expect(calls[0]!.init.method).toBe('POST')
    expect(header(calls[0]!, 'x-c0re-csrf')).toBe('tok123')
    expect(header(calls[0]!, 'content-type')).toBe('application/json')

    mockFetch(200, {})
    await api.updateKey('k_abc123', { reach: 'off' })
    expect(calls[0]!.init.method).toBe('PATCH')
    expect(header(calls[0]!, 'x-c0re-csrf')).toBe('tok123')

    mockFetch(204, null)
    const del = await api.deleteKey('k_abc123')
    expect(del.ok).toBe(true)
    expect(calls[0]!.init.method).toBe('DELETE')
    expect(header(calls[0]!, 'x-c0re-csrf')).toBe('tok123')

    mockFetch(200, [])
    await api.keys()
    expect(header(calls[0]!, 'x-c0re-csrf')).toBeNull()
  })

  it('maps the error body to code and message', async () => {
    mockFetch(403, { error: { code: 'bad_csrf', message: 'Reload the page.' } })
    const r = await api.keys()
    expect(r).toEqual({ ok: false, status: 403, code: 'bad_csrf', message: 'Reload the page.' })
  })

  it('reports 401 as unauthorized', async () => {
    mockFetch(401, { error: { code: 'not_logged_in', message: 'Please log in.' } })
    const r = await api.overview()
    expect(r.ok).toBe(false)
    if (!r.ok) expect(r.code).toBe('unauthorized')
  })

  it('keeps the csrf from login and from status', async () => {
    mockFetch(200, { csrf: 'fromlogin' })
    await api.login('pw')
    mockFetch(200, {})
    await api.setPublic(true)
    expect(header(calls[0]!, 'x-c0re-csrf')).toBe('fromlogin')

    mockFetch(200, { password_set: true, logged_in: true, csrf: 'fromstatus' })
    await api.status()
    mockFetch(200, {})
    await api.setPublic(false)
    expect(header(calls[0]!, 'x-c0re-csrf')).toBe('fromstatus')
  })

  it('turns network failures into an error result', async () => {
    globalThis.fetch = vi.fn(async () => {
      throw new TypeError('Failed to fetch')
    }) as typeof fetch
    const r = await api.keys()
    expect(r).toMatchObject({ ok: false, code: 'network' })
  })
})
