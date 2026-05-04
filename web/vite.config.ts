import { defineConfig } from 'vite';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  build: {
    target: 'esnext',
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        app: resolve(__dirname, 'app/index.html'),
      },
      output: {
        manualChunks: {
          three: ['three'],
          codemirror: [
            'codemirror',
            '@codemirror/view',
            '@codemirror/state',
            '@codemirror/language',
            '@codemirror/autocomplete',
            '@codemirror/commands',
            '@codemirror/search',
            '@codemirror/lint',
            '@lezer/common',
            '@lezer/highlight',
          ],
        },
      },
    },
  },
  optimizeDeps: {
    exclude: ['ferncad-wasm'],
  },
  server: {
    fs: {
      allow: ['..'],
    },
  },
});
