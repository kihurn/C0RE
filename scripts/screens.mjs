// Dashboard screenshots against sample API data (no server needed).
// Usage: PW=<path to playwright> node scripts/screens.mjs <out-dir>
import { randomBytes } from 'node:crypto'
import { readFileSync, existsSync, mkdirSync } from 'node:fs'
import { createRequire } from 'node:module'
import { extname, isAbsolute, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PW ?? 'playwright')
const root = fileURLToPath(new URL('..', import.meta.url))
const dist = resolve(root, 'web/dist')

// Only ever reads and writes inside the project.
const inside = (base, p) => {
  const rel = relative(base, p)
  return rel === '' || (rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel))
}
const out = resolve(root, process.argv[2] ?? 'screens')
if (!inside(root, out)) {
  console.error(`The output folder must be inside the project (${root}).`)
  process.exit(1)
}
mkdirSync(out, { recursive: true })

// Made-up keys for the pictures, generated fresh each run so none is ever stored here.
const fakeKey = () => 'c0re_' + randomBytes(48).toString('base64').replace(/[^A-Za-z0-9]/g, '').slice(0, 43)

const now = Math.floor(Date.now() / 1000)
const hour = Math.floor(now / 3600) * 3600
const perHour = Array.from({ length: 24 }, (_, i) => ({ hour: hour - (23 - i) * 3600, requests: [0,0,1,0,0,0,0,2,6,11,14,9,4,7,12,15,13,8,5,9,14,10,6,3][i], blocked: i === 15 ? 3 : i === 20 ? 1 : 0 }))
const keys = [
  { id: 'k_lenz01', project: 'lenz', reach: 'private', models: ['qwen3-vl:8b-instruct', 'qwen3-vl:4b-instruct'], rate_per_min: 60, daily_cap: 2000, max_concurrent: 2, max_ctx: 8192, max_body_mb: 20, note: '', created_at: now - 86400 * 3, last_used_at: now - 120, revealable: true },
  { id: 'k_site7q', project: 'study-notes-site', reach: 'public', models: ['qwen3-vl:4b-instruct'], rate_per_min: 20, daily_cap: 500, max_concurrent: 1, max_ctx: 8192, max_body_mb: 8, note: '', created_at: now - 86400, last_used_at: now - 3600 * 5, revealable: true },
  { id: 'k_old2xx', project: 'experiment', reach: 'off', models: [], rate_per_min: 60, daily_cap: 2000, max_concurrent: 2, max_ctx: 8192, max_body_mb: 20, note: '', created_at: now - 86400 * 20, last_used_at: null, revealable: false }
]
const recent = [
  { ts: now - 40, key_id: 'k_lenz01', project: 'lenz', entrance: 'private', path: '/api/chat', model: 'qwen3-vl:8b-instruct', status: 200, block_reason: null, latency_ms: 1840, ttfb_ms: 310, prompt_tokens: 812, output_tokens: 96 },
  { ts: now - 300, key_id: 'k_site7q', project: 'study-notes-site', entrance: 'public', path: '/api/chat', model: 'qwen3-vl:4b-instruct', status: 200, block_reason: null, latency_ms: 940, ttfb_ms: 150, prompt_tokens: 220, output_tokens: 61 },
  { ts: now - 620, key_id: 'k_lenz01', project: 'lenz', entrance: 'public', path: '/api/chat', model: 'qwen3-vl:8b-instruct', status: 403, block_reason: 'key_not_public', latency_ms: 2, ttfb_ms: null, prompt_tokens: null, output_tokens: null },
  { ts: now - 1300, key_id: null, project: null, entrance: 'public', path: '/api/chat', model: null, status: 401, block_reason: 'invalid_key', latency_ms: 1, ttfb_ms: null, prompt_tokens: null, output_tokens: null }
]
const api = {
  status: { password_set: true, logged_in: true, csrf: 'x' },
  overview: { engine: { online: true, version: '0.40.13', loaded: [{ name: 'qwen3-vl:8b-instruct', size: 6100000000, size_vram: 6100000000, gpu_percent: 100, context: 8192 }] }, today: { requests: 142, blocked: 4, projects: 2 }, per_hour: perHour, recent, keys: 3, entrances: { private: { requests: 128, blocked: 1 }, public: { requests: 14, blocked: 3 } }, gpu: { name: 'NVIDIA GeForce RTX 4070 SUPER', util_percent: 62, mem_used_mb: 7340, mem_total_mb: 12282, temp_c: 58, power_w: 164 } },
  keys,
  models: [
    { name: 'qwen3-vl:8b-instruct', size: 6100000000, loaded: true, vision: true, thinking: false, capabilities: ['completion', 'vision'] },
    { name: 'qwen3-vl:4b-instruct', size: 3300000000, loaded: false, vision: true, thinking: false, capabilities: ['completion', 'vision'] },
    { name: 'qwen3-vl:8b', size: 6100000000, loaded: false, vision: true, thinking: true, capabilities: ['completion', 'vision', 'thinking'] }
  ],
  usage: { days: 7, summary: { total: 913, blocked: 22, by_reason: [['invalid_key', 12], ['key_not_public', 6], ['rate_limited', 4]], per_key: [
    { key_id: 'k_lenz01', project: 'lenz', requests: 804, blocked: 6, avg_latency_ms: 1730, prompt_tokens: 512340, output_tokens: 70210 },
    { key_id: 'k_site7q', project: 'study-notes-site', requests: 97, blocked: 4, avg_latency_ms: 910, prompt_tokens: 21400, output_tokens: 6020 }], per_hour: perHour } },
  settings: { private_defaults: { rate_per_min: 60, daily_cap: 2000, max_concurrent: 2, max_ctx: 8192, max_body_mb: 20 }, public_defaults: { rate_per_min: 20, daily_cap: 500, max_concurrent: 1, max_ctx: 8192, max_body_mb: 8 }, retention_days: 90, max_db_mb: 100 },
  public: { state: 'on', url: 'https://kihurn-desktop.tail1234.ts.net:8443' }
}
const types = { '.woff2': 'font/woff2', '.woff': 'font/woff', '.js': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.svg': 'image/svg+xml' }

const shots = [
  ['overview', '/admin/'],
  ['keys', '/admin/keys'],
  ['usage', '/admin/usage'],
  ['models', '/admin/models'],
  ['settings', '/admin/settings'],
  ['secret', '/admin/keys'],
  ['reveal', '/admin/keys']
]
const sizes = [[1180, 820, 'desktop'], [390, 844, 'phone']]
const phoneShots = ['overview', 'keys', 'reveal']

// Serves the built dashboard and the sample API to one page.
async function serve(route) {
  const url = new URL(route.request().url())
  if (url.pathname.startsWith('/admin/api/')) {
    const name = url.pathname.slice('/admin/api/'.length).split('/')[0]
    if (name === 'keys' && route.request().method() === 'POST') {
      return route.fulfill({ json: { key: { ...keys[0], id: 'k_new123', project: 'lenz-laptop' }, secret: fakeKey() } })
    }
    if (url.pathname.endsWith('/reveal')) return route.fulfill({ json: { secret: fakeKey() } })
    return route.fulfill({ json: api[name] ?? {} })
  }
  let file = resolve(dist, url.pathname.slice('/admin/'.length))
  if (!inside(dist, file) || !existsSync(file) || file === dist) file = join(dist, 'index.html')
  return route.fulfill({ body: readFileSync(file), contentType: types[extname(file)] ?? 'application/octet-stream' })
}

// One screenshot, in its own page, so they can all run at once.
async function shoot(browser, [name, path], [w, h, tag]) {
  const page = await browser.newPage({ viewport: { width: w, height: h }, colorScheme: 'dark', deviceScaleFactor: 2 })
  await page.route('**/admin/**', serve)
  await page.goto(`http://c0re.test${path}`)
  await page.waitForTimeout(2000)
  if (name === 'secret') {
    await page.getByRole('button', { name: 'New key' }).click()
    await page.getByLabel('Project name').fill('lenz-laptop')
    await page.getByRole('button', { name: 'Create key' }).click()
    await page.waitForTimeout(200)
  } else if (name === 'settings') {
    await page.getByRole('button', { name: 'Save settings' }).hover()
    await page.waitForTimeout(150)
  } else if (name === 'reveal') {
    await page.getByRole('button', { name: "Show lenz's key" }).click()
    await page.waitForTimeout(200)
  }
  await page.screenshot({ path: join(out, `${name}-${tag}-dark.png`), fullPage: name !== 'secret' })
  await page.close()
}

const browser = await chromium.launch()
const jobs = sizes.flatMap((size) => shots.filter(([name]) => size[2] === 'desktop' || phoneShots.includes(name)).map((shot) => [shot, size]))
await Promise.all(jobs.map(([shot, size]) => shoot(browser, shot, size)))
await browser.close()
console.log('screenshots in', out)
