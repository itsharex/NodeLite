<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useOverviewStore } from '@/stores/overview';
import { useNodesStore } from '@/stores/nodes';
import { effectiveGeoLocation } from '@/lib/nodeMeta';

const store = useOverviewStore();
const nodesStore = useNodesStore();

const PLACEHOLDER = '--';

const now = ref(new Date());

let clockTimer: number | undefined;

const total = computed(() => (store.data ? String(store.data.total_nodes) : PLACEHOLDER));
const onlineRatio = computed(() =>
  store.data ? `${store.data.online_nodes} / ${store.data.total_nodes}` : PLACEHOLDER,
);
const onlineFill = computed(() => {
  const totalNodes = store.data?.total_nodes ?? 0;
  if (totalNodes === 0) return 0;
  return Math.max(0, Math.min(100, ((store.data?.online_nodes ?? 0) / totalNodes) * 100));
});
const offline = computed(() => (store.data ? String(store.data.offline_nodes) : PLACEHOLDER));
const time = computed(() =>
  new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(now.value),
);
const regions = computed(() => {
  if (!store.data && nodesStore.nodes.length === 0) return PLACEHOLDER;
  const names = new Set(
    nodesStore.nodes
      .map((node) => {
        const location = effectiveGeoLocation(node);
        return location.city || location.country;
      })
      .filter((name): name is string => Boolean(name)),
  );
  return String(names.size);
});
const averageLoad = computed(() => {
  const values = nodesStore.nodes
    .map((node) => node.snapshot?.load.one)
    .filter((value): value is number => value != null && Number.isFinite(value));
  if (values.length === 0) return PLACEHOLDER;
  const avg = values.reduce((sum, value) => sum + value, 0) / values.length;
  return avg.toFixed(2);
});

onMounted(() => {
  clockTimer = window.setInterval(() => {
    now.value = new Date();
  }, 1000);
});

onUnmounted(() => {
  if (clockTimer !== undefined) window.clearInterval(clockTimer);
});
</script>

<template>
  <div class="stats-grid" data-test="overview-stats">
    <article class="stat-card clock">
      <div class="card-top">
        <span class="stat-icon time" />
        <div class="label">{{ $t('index.stat.time') }}</div>
      </div>
      <div class="value" data-test="stat-time">{{ time }}</div>
      <div class="clock-bars" aria-hidden="true">
        <span />
        <span />
        <span />
        <span />
      </div>
    </article>

    <article class="stat-card online">
      <div class="card-top">
        <span class="stat-icon online" />
        <div class="label">{{ $t('index.stat.online_ratio') }}</div>
      </div>
      <div class="value" data-test="stat-online">{{ onlineRatio }}</div>
      <div class="online-bar" aria-hidden="true">
        <span :style="{ width: `${onlineFill}%` }" />
      </div>
    </article>

    <article class="stat-card regions">
      <div class="card-top">
        <span class="stat-icon regions" />
        <div class="label">{{ $t('index.stat.regions') }}</div>
      </div>
      <div class="value" data-test="stat-regions">{{ regions }}</div>
      <div class="region-dots" aria-hidden="true">
        <span v-for="i in 7" :key="i" />
      </div>
    </article>

    <article class="stat-card total">
      <div class="card-top">
        <span class="stat-icon total" />
        <div class="label">{{ $t('index.stat.total') }}</div>
      </div>
      <div class="value" data-test="stat-total">{{ total }}</div>
    </article>

    <article class="stat-card offline">
      <div class="card-top">
        <span class="stat-icon offline" />
        <div class="label">{{ $t('index.stat.offline') }}</div>
      </div>
      <div class="value" data-test="stat-offline">{{ offline }}</div>
    </article>

    <article class="stat-card load">
      <div class="card-top">
        <span class="stat-icon load" />
        <div class="label">{{ $t('index.stat.avg_load') }}</div>
      </div>
      <div class="value accent" data-test="stat-avg-load">{{ averageLoad }}</div>
    </article>
  </div>
</template>

<style scoped>
.stats-grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 12px;
}
.stat-card {
  background: var(--bg-card);
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  box-shadow: var(--panel-shadow);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 118px;
  overflow: hidden;
  position: relative;
}
.stat-card::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: inherit;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
  pointer-events: none;
}
.card-top {
  display: flex;
  align-items: center;
  gap: 10px;
}
.stat-card .label {
  color: var(--text-secondary);
  font-size: 13px;
  font-weight: 600;
}
.stat-icon {
  width: 24px;
  height: 24px;
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.08), transparent), var(--bg-card-soft);
  flex: 0 0 auto;
  position: relative;
}
.stat-icon::before,
.stat-icon::after {
  content: '';
  position: absolute;
}
.stat-icon.time::before {
  border: 2px solid var(--text-muted);
  border-radius: 50%;
  inset: 5px;
}
.stat-icon.time::after {
  background: var(--text-muted);
  border-radius: 999px;
  height: 6px;
  left: 11px;
  top: 7px;
  transform-origin: bottom;
  transform: rotate(-35deg);
  width: 2px;
}
.stat-icon.online::before,
.stat-icon.load::before {
  background: currentColor;
  border-radius: 999px;
  height: 2px;
  left: 5px;
  right: 5px;
  top: 11px;
}
.stat-icon.online {
  color: var(--accent-green);
}
.stat-icon.load {
  color: var(--accent-blue);
}
.stat-icon.regions::before,
.stat-icon.total::before,
.stat-icon.offline::before {
  border-radius: 50%;
  height: 8px;
  width: 8px;
}
.stat-icon.regions::before {
  background: var(--accent-blue);
  box-shadow:
    9px 0 0 rgba(111, 140, 255, 0.45),
    4px 8px 0 rgba(111, 140, 255, 0.7);
  left: 4px;
  top: 4px;
}
.stat-icon.total::before {
  border: 2px solid var(--text-muted);
  left: 6px;
  top: 6px;
}
.stat-icon.offline::before {
  background: var(--accent-red);
  box-shadow: 0 0 16px rgba(255, 77, 109, 0.55);
  left: 7px;
  top: 7px;
}
.stat-card .value {
  font-size: clamp(24px, 2vw, 32px);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
.stat-card .value.accent,
.stat-card.online .value {
  color: var(--accent-green);
}
.stat-card.offline .value {
  color: var(--accent-red);
}
.stat-card.regions .value {
  color: var(--accent-blue);
}
.clock-bars,
.online-bar,
.region-dots {
  margin-top: auto;
}
.clock-bars {
  display: flex;
  gap: 8px;
}
.clock-bars span,
.online-bar {
  height: 5px;
  border-radius: 999px;
  background: var(--bg-card-soft);
}
.clock-bars span {
  flex: 1;
}
.clock-bars span:nth-child(odd) {
  background: var(--accent-blue);
}
.clock-bars span:nth-child(even) {
  background: var(--accent-green);
}
.online-bar {
  overflow: hidden;
}
.online-bar span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent-green);
}
.region-dots {
  display: flex;
  gap: 8px;
}
.region-dots span {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--accent-blue);
  box-shadow: 0 0 18px rgba(111, 140, 255, 0.5);
  opacity: 0.72;
}
@media (max-width: 1120px) {
  .stats-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
@media (max-width: 720px) {
  .stats-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 440px) {
  .stats-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
