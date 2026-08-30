<template>
  <div class="version-flow-arrow" aria-hidden="true">
    <span class="version-flow-arrow__glyph" />
    <span class="version-flow-arrow__flow" />
  </div>
</template>

<style scoped>
.version-flow-arrow {
  --version-arrow-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'%3E%3Cpath d='M5 12h14M12 5l7 7-7 7' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");

  position: relative;
  display: grid;
  width: 2.75rem;
  height: 2.75rem;
  color: var(--foreground);
  place-items: center;
  isolation: isolate;
}

.version-flow-arrow::before {
  z-index: -1;
  width: 1.75rem;
  height: 1.1rem;
  grid-area: 1 / 1;
  border-radius: 999px;
  background: conic-gradient(
    from 90deg,
    #ff4f9a,
    #ffe66d,
    #4fffc1,
    #45caff,
    #8b7cff,
    #ff5fd2,
    #ff4f9a
  );
  content: "";
  opacity: 0;
  filter: blur(0.5rem) saturate(1.25);
  transform: scale(0.72);
  transition:
    opacity 600ms cubic-bezier(0.22, 1, 0.36, 1),
    transform 700ms cubic-bezier(0.16, 1, 0.3, 1);
  animation: version-arrow-aura 6s linear infinite paused;
}

.version-flow-arrow__flow,
.version-flow-arrow__glyph {
  width: 1.5rem;
  height: 1.5rem;
  grid-area: 1 / 1;
  -webkit-mask: var(--version-arrow-mask) center / contain no-repeat;
  mask: var(--version-arrow-mask) center / contain no-repeat;
}

.version-flow-arrow__glyph {
  background: currentColor;
  transition: opacity 450ms cubic-bezier(0.22, 1, 0.36, 1);
}

.version-flow-arrow__flow {
  background-image: linear-gradient(
    110deg,
    #ff4f9a 0%,
    #ff9f43 16%,
    #ffe66d 31%,
    #4fffc1 48%,
    #45caff 65%,
    #8b7cff 82%,
    #ff5fd2 100%
  );
  background-size: 175% 100%;
  opacity: 0;
  filter: saturate(1.05) drop-shadow(0 0 0 rgb(110 196 255 / 0%));
  transform: scale(0.92);
  transition:
    opacity 500ms cubic-bezier(0.22, 1, 0.36, 1),
    filter 600ms ease,
    transform 650ms cubic-bezier(0.16, 1, 0.3, 1);
  animation: version-arrow-flow 3.8s linear infinite paused;
}

.version-flow-arrow:hover .version-flow-arrow__glyph {
  opacity: 0;
}

.version-flow-arrow:hover::before {
  opacity: 0.26;
  transform: scale(1.08);
  animation-play-state: running;
}

.version-flow-arrow:hover .version-flow-arrow__flow {
  opacity: 1;
  filter: saturate(1.15) drop-shadow(0 0 0.22rem rgb(125 183 255 / 48%));
  transform: scale(1);
  animation-play-state: running;
}

@keyframes version-arrow-flow {
  from {
    background-position: 0% 50%;
  }

  to {
    background-position: 150% 50%;
  }
}

@keyframes version-arrow-aura {
  to {
    rotate: 1turn;
  }
}

@media (prefers-reduced-motion: reduce) {
  .version-flow-arrow::before,
  .version-flow-arrow__flow,
  .version-flow-arrow__glyph {
    animation: none;
    background-position: 50% 50%;
  }

  .version-flow-arrow__flow,
  .version-flow-arrow__glyph,
  .version-flow-arrow::before {
    transition-duration: 0.01ms;
  }
}
</style>
