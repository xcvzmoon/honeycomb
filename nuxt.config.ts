import tailwindcss from '@tailwindcss/vite';

export default defineNuxtConfig({
  compatibilityDate: '2026-06-26',
  devtools: {
    enabled: true,
  },
  vite: {
    plugins: [tailwindcss()],
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
