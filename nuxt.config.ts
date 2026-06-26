import tailwindcss from '@tailwindcss/vite';

export default defineNuxtConfig({
  compatibilityDate: '2026-06-26',
  ssr: false,
  devtools: {
    enabled: true,
  },
  devServer: {
    host: '0',
    port: 3000,
  },
  vite: {
    clearScreen: false,
    plugins: [tailwindcss()],
    envPrefix: ['VITE_', 'TAURI_'],
    server: {
      strictPort: true,
    },
    optimizeDeps: {
      include: ['vue', 'vue-router', '@vue/devtools-core', '@vue/devtools-kit'],
    },
  },
  css: ['~/assets/css/main.css'],
  modules: ['@nuxt/ui', '@vueuse/nuxt', '@nuxt/hints', '@nuxt/test-utils'],
  colorMode: {
    preference: 'dark',
    fallback: 'dark',
  },
});
