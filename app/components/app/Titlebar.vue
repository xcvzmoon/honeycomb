<script setup lang="ts">
  import { name, version } from '~~/package.json';
</script>

<template>
  <div
    class="flex h-8.25 items-center gap-2 border-b border-muted bg-muted ps-20 select-none"
    data-tauri-drag-region
  >
    <p class="text-xs">
      {{ capitalize(name) }}
    </p>

    <UBadge
      :label="version"
      size="xs"
      variant="subtle"
    />
  </div>
</template>
