import { defineConfig } from 'vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import viteReact from '@vitejs/plugin-react'
import { cloudflare } from '@cloudflare/vite-plugin'

export default defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [
    cloudflare({ viteEnvironment: { name: 'ssr' } }),
    // The page is static: render it to HTML at build time, served as
    // Workers static assets.
    tanstackStart({ prerender: { enabled: true, crawlLinks: true } }),
    viteReact(),
  ],
})
