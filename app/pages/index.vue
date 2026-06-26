<script setup lang="ts">
  import { name } from '~~/package.json';

  async function navigateToDashboard() {
    await navigateTo('/dashboard');
  }
</script>

<template>
  <section class="flex h-svh flex-col place-items-center items-center justify-center gap-4">
    <p class="font-mono">{{ `Welcome to ${capitalize(name)}` }}</p>

    <UButton
      label="Continue"
      trailing-icon="i-lucide-move-right"
      @click="navigateToDashboard"
    />
  </section>
</template>
