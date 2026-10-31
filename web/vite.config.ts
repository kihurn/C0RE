import { svelte } from '@sveltejs/vite-plugin-svelte'
import { readFileSync } from 'node:fs'
import { defineConfig } from 'vite'

// `npm run dev` serves the dashboard at http://localhost:5173/admin/ with live reload and
// sends its API calls to a running C0RE (your PC by default; set C0RE_URL to change it).
const c0re = process.env.C0RE_URL ?? 'http://kihurn-desktop:7080'
// The version shown in the dashboard comes from Cargo.toml, so the two never disagree.
const version = /^version = "([^"]+)"/m.exec(readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8'))?.[1] ?? 'dev'

export default defineConfig({
  base: '/admin/',
  plugins: [svelte()],
  define: { __C0RE_VERSION__: JSON.stringify(version) },
  server: { proxy: { '/admin/api': { target: c0re, changeOrigin: true } } },
  build: { outDir: 'dist', emptyOutDir: true, assetsInlineLimit: 0 },
  test: { include: ['test/**/*.test.ts'], environment: 'node' }
})
