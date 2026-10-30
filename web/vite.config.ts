import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

export default defineConfig({
  base: '/admin/',
  plugins: [svelte()],
  build: { outDir: 'dist', emptyOutDir: true, assetsInlineLimit: 0 },
  test: { include: ['test/**/*.test.ts'], environment: 'node' }
})
