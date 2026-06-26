<script setup lang="ts">
  type Post = {
    id: number;
    userId: number;
    title: string;
    body: string;
  };

  definePageMeta({ layout: 'authenticated' });

  const inChatSelectionMode = ref<boolean>(false);
  const searchChatInput = ref<string>('');
  const selectedChats = ref<string[]>([]);

  // test only
  const { data: posts } = await useAsyncData('posts', async () => {
    return $fetch<Post[]>('/posts', {
      baseURL: 'https://jsonplaceholder.typicode.com/',
    });
  });

  const chatList = computed(() => {
    const chatListItems = posts.value?.map(({ title }) => title) ?? [];
    return chatListItems.filter((item) => item.startsWith(searchChatInput.value));
  });
</script>

<template>
  <div class="space-y-4 p-4">
    <div class="flex items-center gap-2">
      <div
        v-if="inChatSelectionMode"
        class="flex items-center gap-4"
      >
        <UButton
          color="neutral"
          label="Select all"
        />

        <p class="text-xs text-muted">{{ `${selectedChats.length} selected` }}</p>
      </div>

      <div class="ml-auto space-x-2">
        <UButton
          v-if="inChatSelectionMode"
          :disabled="selectedChats.length === 0"
          color="error"
          variant="soft"
          label="Delete"
        />

        <UButton
          :color="inChatSelectionMode ? 'neutral' : 'primary'"
          :label="inChatSelectionMode ? 'Cancel' : 'Select chats'"
          variant="soft"
          @click="inChatSelectionMode = !inChatSelectionMode"
        />
      </div>
    </div>

    <UInput
      v-model="searchChatInput"
      icon="i-lucide-search"
      placeholder="Search chats..."
      class="w-full"
    />

    <AppScrollArea class="h-[calc(100svh-14.125rem)]">
      <UCheckboxGroup
        v-if="inChatSelectionMode"
        v-model="selectedChats"
        :items="chatList"
      />

      <ul
        v-else
        class="divide-y divide-muted"
      >
        <li
          v-for="chatListItem in chatList"
          :key="chatListItem"
          class="p-2"
        >
          <UButton
            color="neutral"
            variant="ghost"
            class="w-full justify-between text-muted hover:text-default"
          >
            <span>{{ chatListItem }}</span>
            <span>1 hr ago</span>
          </UButton>
        </li>
      </ul>
    </AppScrollArea>
  </div>
</template>
