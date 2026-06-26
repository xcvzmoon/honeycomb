<template>
  <div class="flex flex-col bg-default">
    <AppTitlebar />

    <div class="flex h-[calc(100svh-2.0625rem)]">
      <AppSidebar />

      <div class="flex-1">
        <AppHeader />

        <AppScrollArea class="h-[calc(100svh-6.125rem)]">
          <NuxtPage />
        </AppScrollArea>
      </div>
    </div>
  </div>
</template>
