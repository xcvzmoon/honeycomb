<script setup lang="ts">
  definePageMeta({ layout: 'authenticated' });

  const route = useRoute();
</script>

<template>
  <div class="p-4">
    <div class="border-l-4 border-primary pl-3">
      <p class="text-lg font-bold uppercase">{{ route.name }}</p>
      <p class="text-sm text-muted">Manage and organize your learning course</p>
    </div>
  </div>
</template>
