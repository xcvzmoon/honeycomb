<script setup lang="ts">
  const sidebar = useSidebar();
  const { isSidebarCollapsed } = sidebar;

  const routes = [
    { to: '/dashboard', icon: 'i-lucide-layout-dashboard', label: 'Dashboard' },
    { to: '/courses', icon: 'i-lucide-library', label: 'Courses' },
    { to: '/chats', icon: 'i-lucide-messages-square', label: 'Chats' },
    { to: '/settings', icon: 'i-lucide-settings', label: 'Settings' },
  ];
</script>

<template>
  <aside
    :class="isSidebarCollapsed ? 'w-16.25' : 'w-62.75'"
    class="border-r border-default bg-muted/25 p-2 transition-all"
  >
    <div class="p-2">
      <UTooltip
        :disabled="!isSidebarCollapsed"
        :delay-duration="0"
        :content="{
          align: 'center',
          side: 'right',
          sideOffset: 8,
        }"
        :ui="{
          content: 'bg-inverted text-inverted',
          arrow: 'fill-inverted stroke-inverted',
        }"
        text="Star new chat"
        arrow
      >
        <UButton class="relative h-8 w-full justify-center px-2 transition-all">
          <UIcon name="i-lucide-plus" />

          <Transition
            enter-active-class="transition-all"
            enter-from-class="max-w-0 -translate-x-1 opacity-0"
            enter-to-class="max-w-32 translate-x-0 opacity-100"
            leave-active-class="absolute transition-all"
            leave-from-class="max-w-32 translate-x-0 opacity-100"
            leave-to-class="max-w-0 -translate-x-1 opacity-0"
          >
            <span
              v-if="!isSidebarCollapsed"
              class="inline-block overflow-hidden align-middle whitespace-nowrap"
            >
              Star new chat
            </span>
          </Transition>
        </UButton>
      </UTooltip>
    </div>

    <ul class="mt-8 space-y-1 p-2">
      <li
        v-for="route in routes"
        :key="route.to"
      >
        <UTooltip
          :disabled="!isSidebarCollapsed"
          :text="route.label"
          :delay-duration="0"
          :content="{
            align: 'center',
            side: 'right',
            sideOffset: 8,
          }"
          :ui="{
            content: 'bg-inverted text-inverted',
            arrow: 'fill-inverted stroke-inverted',
          }"
          arrow
        >
          <UButton
            :to="route.to"
            :icon="route.icon"
            :label="route.label"
            :color="$route.path === route.to ? 'primary' : 'neutral'"
            :variant="$route.path === route.to ? 'solid' : 'ghost'"
            class="w-full px-2"
          />
        </UTooltip>
      </li>
    </ul>
  </aside>
</template>
