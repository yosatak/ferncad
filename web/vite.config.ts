import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    target: 'esnext',
    rollupOptions: {
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
