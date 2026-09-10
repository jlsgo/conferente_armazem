import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Convencoes do Tauri: https://tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    outDir: 'dist',
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // src-tauri/target tem dezenas de milhares de arquivos de build do Rust -
      // sem isso o watcher do Vite estoura o limite de inotify do Linux
      // (ENOSPC) e derruba o `tauri dev` inteiro antes do cargo terminar.
      ignored: ['**/src-tauri/**'],
    },
  },
});
