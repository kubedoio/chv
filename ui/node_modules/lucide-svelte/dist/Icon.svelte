<script>import defaultAttributes from './defaultAttributes';
import { hasA11yProp } from './utils/hasA11yProp';
import { mergeClasses } from './utils/mergeClasses';
export let name = undefined;
export let color = 'currentColor';
export let size = 24;
export let strokeWidth = 2;
export let absoluteStrokeWidth = false;
export let iconNode = [];
</script>

<svg
  {...defaultAttributes}
  {...(!hasA11yProp($$restProps) ? {'aria-hidden': 'true'} : undefined)}
  {...$$restProps}
  width={size}
  height={size}
  stroke={color}
  stroke-width={
    absoluteStrokeWidth
      ? Number(strokeWidth) * 24 / Number(size)
      : strokeWidth
  }
  class={
    mergeClasses(
      'lucide-icon',
      'lucide',
      name ? `lucide-${name}`: '',
      $$props.class
    )
  }
>
  {#each iconNode as [tag, attrs]}
    <svelte:element this={tag} {...attrs}/>
  {/each}
  <slot />
</svg>
