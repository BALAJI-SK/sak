import { defineConfig } from 'vite'

export default defineConfig({
  server: {
    host: '0.0.0.0',
    port: 4000,
    proxy: {
      '/api/nvidia': {
        target: 'https://integrate.api.nvidia.com/v1',
        changeOrigin: true,
        rewrite: path => path.replace(/^\/api\/nvidia/, ''),
      },
      '/evaluate': { target: 'http://127.0.0.1:3001', changeOrigin: true },
      '/squads': { target: 'http://127.0.0.1:3001', changeOrigin: true },
      '/feedback': { target: 'http://127.0.0.1:3001', changeOrigin: true },
      '/sol-price': { target: 'http://127.0.0.1:3001', changeOrigin: true },
      '/health': { target: 'http://127.0.0.1:3001', changeOrigin: true },
      '/ws': { target: 'ws://127.0.0.1:3001', ws: true },
    },
  },
})
