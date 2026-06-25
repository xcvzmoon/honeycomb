import { mountSuspended } from '@nuxt/test-utils/runtime';
import { describe, expect, it } from 'vite-plus/test';
import { defineComponent, h } from 'vue';

describe('component test example', () => {
  it('can mount components', async () => {
    const TestComponent = defineComponent({
      setup() {
        return () => h('div', 'Hello Nuxt!');
      },
    });

    const component = await mountSuspended(TestComponent);

    expect(component.text()).toBe('Hello Nuxt!');
  });
});
