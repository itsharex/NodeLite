<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { HistoryPoint, NodeStatus } from '@/api';
import { averageValue, buildChartData, type ChartPoint } from '@/lib/chart/chartData';
import { networkSeries } from '@/lib/chart/svgModel';
import { fmtBytes, fmtLatency, fmtPercent, fmtRate } from '@/lib/format';
import MetricChart from './MetricChart.vue';

const props = defineProps<{ node: NodeStatus | null; history: HistoryPoint[] }>();

const { t } = useI18n();

type Tone = 'ok' | 'warn' | 'bad' | 'neutral';

const chartData = computed(() => buildChartData(props.history));
const trafficSeries = computed(() =>
  networkSeries(chartData.value, t('index.node.download'), t('index.node.upload')),
);

const network = computed(() => props.node?.snapshot?.network ?? null);
const rxRate = computed(() => network.value?.rx_bytes_per_sec ?? null);
const txRate = computed(() => network.value?.tx_bytes_per_sec ?? null);
const rxTotal = computed(() => network.value?.total_rx_bytes ?? null);
const txTotal = computed(() => network.value?.total_tx_bytes ?? null);
const totalTrafficBytes = computed(() => {
  if (rxTotal.value == null && txTotal.value == null) return null;
  return (rxTotal.value ?? 0) + (txTotal.value ?? 0);
});
const activeRate = computed(() => {
  if (rxRate.value == null && txRate.value == null) return null;
  return (rxRate.value ?? 0) + (txRate.value ?? 0);
});
const packetLoss = computed(() => network.value?.packet_loss_percent ?? null);
const averagePacketLoss = computed(() => averageValue(chartData.value.packetLossPts));
const averageLatency = computed(() => averageValue(chartData.value.rttPts));
const peakRate = computed(() => maxPointValue([...chartData.value.dlPts, ...chartData.value.upPts]));
const rxShare = computed(() => {
  const total = totalTrafficBytes.value;
  if (!total || rxTotal.value == null) return 0;
  return Math.max(0, Math.min(100, (rxTotal.value / total) * 100));
});
const txShare = computed(() => (totalTrafficBytes.value ? 100 - rxShare.value : 0));

const statCards = computed(() => [
  {
    key: 'download',
    label: t('index.node.download'),
    value: fmtRate(rxRate.value) ?? '—',
    meta: t('node.network.total_value', { value: fmtBytes(rxTotal.value) ?? '—' }),
    tone: 'ok' as Tone,
  },
  {
    key: 'upload',
    label: t('index.node.upload'),
    value: fmtRate(txRate.value) ?? '—',
    meta: t('node.network.total_value', { value: fmtBytes(txTotal.value) ?? '—' }),
    tone: 'neutral' as Tone,
  },
  {
    key: 'latency',
    label: t('node.network.rtt'),
    value: fmtLatency(props.node?.latency_ms) ?? '—',
    meta: averageLatency.value == null ? t('node.network.avg_empty') : fmtLatency(averageLatency.value),
    tone: latencyTone(props.node?.latency_ms),
  },
  {
    key: 'loss',
    label: t('node.network.packet_loss'),
    value: fmtPercent(packetLoss.value) ?? '—',
    meta:
      averagePacketLoss.value == null
        ? t('node.network.avg_empty')
        : t('node.network.avg_value', { value: fmtPercent(averagePacketLoss.value) ?? '—' }),
    tone: lossTone(packetLoss.value),
  },
]);

const qualityRows = computed(() => [
  {
    label: t('node.network.status'),
    value: props.node?.online ? t('common.online') : t('common.offline'),
    tone: props.node?.online ? 'ok' : 'bad',
  },
  {
    label: t('node.network.avg_rtt'),
    value: fmtLatency(averageLatency.value) ?? '—',
    tone: latencyTone(averageLatency.value),
  },
  {
    label: t('node.network.peak_rate'),
    value: fmtRate(peakRate.value) ?? '—',
    tone: 'neutral',
  },
  {
    label: t('node.network.samples'),
    value: t('node.network.samples_count', { count: props.history.length }),
    tone: 'neutral',
  },
]);

const totalRows = computed(() => [
  { label: t('node.network.received'), value: fmtBytes(rxTotal.value) ?? '—' },
  { label: t('node.network.transmitted'), value: fmtBytes(txTotal.value) ?? '—' },
  { label: t('node.network.total_traffic'), value: fmtBytes(totalTrafficBytes.value) ?? '—' },
  { label: t('node.network.active_rate'), value: fmtRate(activeRate.value) ?? '—' },
]);

function maxPointValue(points: ChartPoint[]): number | null {
  const values = points
    .map((point) => point.value)
    .filter((value): value is number => value != null && Number.isFinite(Number(value)));
  if (values.length === 0) return null;
  return Math.max(...values);
}

function latencyTone(value: number | null | undefined): Tone {
  if (value == null || !Number.isFinite(Number(value))) return 'neutral';
  if (value >= 300) return 'bad';
  if (value >= 180) return 'warn';
  return 'ok';
}

function lossTone(value: number | null | undefined): Tone {
  if (value == null || !Number.isFinite(Number(value))) return 'neutral';
  if (value >= 5) return 'bad';
  if (value >= 1) return 'warn';
  return 'ok';
}
</script>

<template>
  <div class="network-panel" data-test="network-pane">
    <section class="network-stat-grid" data-test="network-stat-grid">
      <article
        v-for="card in statCards"
        :key="card.key"
        class="network-stat"
        :class="`network-stat--${card.tone}`"
        :data-test="`network-stat-${card.key}`"
      >
        <span class="network-stat__label">{{ card.label }}</span>
        <strong>{{ card.value }}</strong>
        <small>{{ card.meta }}</small>
      </article>
    </section>

    <section class="network-layout">
      <article class="network-card traffic-card" data-test="network-traffic-card">
        <header class="network-card__head">
          <div>
            <span class="card-kicker">{{ t('node.network.live') }}</span>
            <strong>{{ t('node.network_traffic') }}</strong>
          </div>
          <div class="traffic-legend" aria-hidden="true">
            <span class="legend-item legend-item--down">{{ t('index.node.download') }}</span>
            <span class="legend-item legend-item--up">{{ t('index.node.upload') }}</span>
          </div>
        </header>
        <MetricChart :series="trafficSeries" value-kind="rate" :min-value="0" :height="260" />
      </article>

      <article class="network-card quality-card" data-test="network-quality-card">
        <header class="network-card__head">
          <div>
            <span class="card-kicker">{{ t('node.network.quality') }}</span>
            <strong>{{ t('node.network.link_health') }}</strong>
          </div>
        </header>
        <div class="quality-meter" :class="`quality-meter--${lossTone(packetLoss)}`">
          <span>{{ t('node.network.packet_loss') }}</span>
          <strong>{{ fmtPercent(packetLoss) ?? '—' }}</strong>
        </div>
        <div class="quality-list">
          <div v-for="row in qualityRows" :key="row.label" class="quality-row">
            <span>{{ row.label }}</span>
            <strong :class="`tone-${row.tone}`">{{ row.value }}</strong>
          </div>
        </div>
      </article>
    </section>

    <section class="network-layout network-layout--bottom">
      <article class="network-card loss-card" data-test="network-loss-card">
        <header class="network-card__head">
          <div>
            <span class="card-kicker">{{ t('node.network.loss_history') }}</span>
            <strong>{{ t('node.network.packet_loss') }}</strong>
          </div>
          <span class="head-value">{{ fmtPercent(averagePacketLoss) ?? '—' }}</span>
        </header>
        <MetricChart
          :points="chartData.packetLossPts"
          value-kind="percent"
          color="var(--accent-red)"
          :min-value="0"
          :max-value="100"
          :height="190"
          :label="t('node.network.packet_loss')"
        />
      </article>

      <article class="network-card totals-card" data-test="network-totals-card">
        <header class="network-card__head">
          <div>
            <span class="card-kicker">{{ t('node.network.totals') }}</span>
            <strong>{{ t('node.network.traffic_mix') }}</strong>
          </div>
          <span class="head-value">{{ fmtBytes(totalTrafficBytes) ?? '—' }}</span>
        </header>
        <div class="traffic-split" aria-hidden="true">
          <span class="traffic-split__rx" :style="{ width: `${rxShare}%` }" />
          <span class="traffic-split__tx" :style="{ width: `${txShare}%` }" />
        </div>
        <div class="totals-list">
          <div v-for="row in totalRows" :key="row.label" class="total-row">
            <span>{{ row.label }}</span>
            <strong>{{ row.value }}</strong>
          </div>
        </div>
      </article>
    </section>
  </div>
</template>

<style scoped>
.network-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.network-stat-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.network-stat,
.network-card {
  background: var(--bg-card);
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  box-shadow: var(--panel-shadow);
}

.network-stat {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 112px;
  padding: 16px;
  position: relative;
  overflow: hidden;
}

.network-stat::before {
  content: '';
  position: absolute;
  inset: 0;
  border-left: 3px solid var(--text-dim);
  opacity: 0.8;
  pointer-events: none;
}

.network-stat--ok::before {
  border-left-color: var(--accent-green);
}

.network-stat--warn::before {
  border-left-color: var(--accent-yellow);
}

.network-stat--bad::before {
  border-left-color: var(--accent-red);
}

.network-stat__label {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 600;
}

.network-stat strong {
  color: var(--text-primary);
  font-size: 24px;
  font-variant-numeric: tabular-nums;
  font-weight: 650;
  line-height: 1.15;
}

.network-stat small {
  color: var(--text-muted);
  font-size: 12px;
  min-height: 18px;
}

.network-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.65fr) minmax(260px, 0.85fr);
  gap: 16px;
}

.network-layout--bottom {
  grid-template-columns: minmax(0, 1.2fr) minmax(300px, 0.8fr);
}

.network-card {
  min-width: 0;
  padding: 16px;
}

.network-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}

.network-card__head > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}

.card-kicker {
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0;
  text-transform: uppercase;
}

.network-card__head strong {
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 650;
}

.traffic-legend {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
  color: var(--text-muted);
  font-size: 12px;
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.legend-item::before {
  content: '';
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: currentColor;
}

.legend-item--down {
  color: var(--chart-network-down);
}

.legend-item--up {
  color: var(--chart-network-up);
}

.quality-card,
.totals-card {
  display: flex;
  flex-direction: column;
}

.quality-meter {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 12px;
  min-height: 102px;
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  background: var(--bg-card-soft);
  padding: 18px;
}

.quality-meter span {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 600;
}

.quality-meter strong {
  font-size: 34px;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}

.quality-meter--ok strong {
  color: var(--accent-green);
}

.quality-meter--warn strong {
  color: var(--accent-yellow);
}

.quality-meter--bad strong {
  color: var(--accent-red);
}

.quality-list,
.totals-list {
  display: flex;
  flex-direction: column;
  margin-top: 14px;
}

.quality-row,
.total-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-top: 1px solid var(--border-soft);
  color: var(--text-muted);
  font-size: 13px;
  padding: 12px 0;
}

.quality-row strong,
.total-row strong {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  font-weight: 650;
  text-align: right;
}

.tone-ok {
  color: var(--accent-green) !important;
}

.tone-warn {
  color: var(--accent-yellow) !important;
}

.tone-bad {
  color: var(--accent-red) !important;
}

.head-value {
  color: var(--text-secondary);
  flex: 0 0 auto;
  font-size: 18px;
  font-variant-numeric: tabular-nums;
  font-weight: 650;
}

.traffic-split {
  display: flex;
  width: 100%;
  height: 14px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--bg-card-soft);
  border: 1px solid var(--border-soft);
}

.traffic-split__rx {
  background: var(--chart-network-down);
}

.traffic-split__tx {
  background: var(--chart-network-up);
}

@media (max-width: 980px) {
  .network-stat-grid,
  .network-layout,
  .network-layout--bottom {
    grid-template-columns: 1fr 1fr;
  }

  .traffic-card,
  .loss-card {
    grid-column: 1 / -1;
  }
}

@media (max-width: 620px) {
  .network-stat-grid,
  .network-layout,
  .network-layout--bottom {
    grid-template-columns: 1fr;
  }

  .network-stat strong {
    font-size: 22px;
  }

  .network-card__head {
    flex-direction: column;
  }
}
</style>
