import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath } from 'url'
import { dirname, resolve } from 'path'

const rootDir = dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [vue()],
  base: './',
  build: {
    outDir: 'dist',
    rollupOptions: {
      // 多页：主窗口（书架）与悬浮阅读窗口
      input: {
        index: resolve(rootDir, 'index.html'),
        reader: resolve(rootDir, 'src/reader/index.html')
      }
    }
  }
})
