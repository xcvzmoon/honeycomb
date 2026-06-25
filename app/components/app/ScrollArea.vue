<script lang="ts" setup>
  import type { HTMLAttributes } from 'vue';
  import {
    type ScrollAreaRootProps,
    ScrollAreaRoot,
    ScrollAreaScrollbar,
    ScrollAreaThumb,
    ScrollAreaViewport,
  } from 'reka-ui';

  type Props = ScrollAreaRootProps & {
    class?: HTMLAttributes['class'];
    mount?: 'vertical' | 'horizontal' | 'both';
  };

  const props = defineProps<Props>();

  const delegatedProps = computed(() => {
    const { class: _, mount: __, ...delegated } = props;
    return delegated;
  });

  const isVertical = computed(() => ['vertical', 'both'].includes(props.mount ?? ''));
  const isHorizontal = computed(() => ['horizontal', 'both'].includes(props.mount ?? ''));

  const thumbClass = `z-20 flex-1 bg-zinc-500 rounded-[10px] relative transition-colors hover:bg-zinc-600 dark:hover:bg-zinc-400 before:content-[''] before:absolute before:top-1/2 before:left-1/2 before:-translate-x-1/2 before:-translate-y-1/2 before:w-full before:h-full before:min-w-[44px] before:min-h-[44px]`;

  const scrollbarClass =
    'z-20 flex touch-none bg-transparent p-0.5 transition-colors duration-160 ease-out data-[orientation=horizontal]:h-2.5 data-[orientation=horizontal]:flex-col data-[orientation=vertical]:w-2.5';
</script>

<template>
  <ScrollAreaRoot
    v-bind="delegatedProps"
    :class="['relative overflow-hidden', props.class]"
  >
    <ScrollAreaViewport class="size-full rounded-[inherit] focus:outline-primary">
      <slot />
    </ScrollAreaViewport>

    <ScrollAreaScrollbar
      :force-mount="isVertical"
      :class="scrollbarClass"
      orientation="vertical"
    >
      <ScrollAreaThumb :class="thumbClass" />
    </ScrollAreaScrollbar>

    <ScrollAreaScrollbar
      :force-mount="isHorizontal"
      :class="scrollbarClass"
      orientation="horizontal"
    >
      <ScrollAreaThumb :class="thumbClass" />
    </ScrollAreaScrollbar>
  </ScrollAreaRoot>
</template>
