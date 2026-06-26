<script setup lang="ts">
  const sidebar = useSidebar();
  const { isSidebarCollapsed, toggleSidebar } = sidebar;

  const route = useRoute();

  const routeName = computed(() => capitalize(route.name?.toString() ?? ''));
</script>

<template>
  <header class="flex items-center gap-4 border-b border-default p-4">
    <UButton
      :icon="isSidebarCollapsed ? 'i-lucide-panel-left-open' : 'i-lucide-panel-left-close'"
      size="icon"
      color="neutral"
      variant="ghost"
      @click="toggleSidebar"
    />

    <UBadge
      :label="routeName"
      color="neutral"
      variant="soft"
    />

    <USeparator
      orientation="vertical"
      class="h-4 rotate-15"
    />

    <div class="ml-auto">
      <UButton
        v-if="route.name === 'courses'"
        icon="i-lucide-plus"
        label="Add new course"
      />

      <UButton
        v-else-if="route.name === 'chats'"
        icon="i-lucide-message-square-plus"
        label="Start new chat"
      />
    </div>
  </header>
</template>
