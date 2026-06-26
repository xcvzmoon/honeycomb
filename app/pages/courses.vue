<script setup lang="ts">
  definePageMeta({ layout: 'authenticated' });

  const FILTER_OPTIONS = ['Recent', 'Alphabetical', 'Goal'];

  const route = useRoute();

  const selectedFilter = ref<string>(FILTER_OPTIONS[0] ?? 'Recent');
</script>

<template>
  <div class="p-4">
    <div class="flex items-end justify-between">
      <div class="border-l-4 border-primary pl-3">
        <p class="text-lg font-bold uppercase">{{ route.name }}</p>
        <p class="text-sm text-muted">Manage and organize your learning course</p>
      </div>

      <div class="flex items-center gap-2">
        <UInput
          icon="i-lucide-search"
          placeholder="Search course..."
        />

        <USelect
          v-model="selectedFilter"
          :items="FILTER_OPTIONS"
          icon="i-lucide-funnel"
          class="w-50"
        />
      </div>
    </div>

    <div class="mt-4">
      <UCard>
        <template #header>
          <Placeholder class="h-8" />
        </template>

        <Placeholder class="h-32" />

        <template #footer>
          <Placeholder class="h-8" />
        </template>
      </UCard>
    </div>
  </div>
</template>
