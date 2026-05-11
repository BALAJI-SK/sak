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
    },
  },
})
