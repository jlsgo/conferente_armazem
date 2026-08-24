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
  },
});
