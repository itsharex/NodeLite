<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { AlertsDraft } from '@/lib/alertsDraft';

/** Top card: global alert enable toggle + a read-only summary of the draft. */
const draft = defineModel<AlertsDraft>({ required: true });

const { t } = useI18n();

const channelSummary = computed(() => {
  const enabled = [
    draft.value.smtp.enabled ? t('alerts.channel.smtp') : '',
    draft.value.webhook.enabled ? t('alerts.channel.webhook') : '',
  ].filter(Boolean);
  return enabled.length ? enabled.join(' + ') : t('settings.disabled');
});

const ruleSummary = computed(() => {
  const total = draft.value.rules.length;
  if (!total) return t('alerts.rules.empty_short');
  const enabled = draft.value.rules.filter((rule) => rule.enabled).length;
  return `${enabled}/${total}`;
});

const inspectionSummary = computed(() => {
  const insp = draft.value.inspection;
  if (!insp.enabled) return t('settings.disabled');
  const delivery = insp.delivery.length
    ? insp.delivery.map((c) => t(`alerts.channel.${c}`)).join(' + ')
    : t('common.not_available');
  return `${insp.local_time || '09:00'} · ${insp.lookback_hours || 24}h · ${delivery}`;
});

const tiles = computed(() => [
  { label: t('alerts.summary.channels'), value: channelSummary.value },
  { label: t('alerts.summary.rules'), value: ruleSummary.value },
  { label: t('alerts.summary.inspection'), value: inspectionSummary.value },
]);

function setEnabled(event: Event): void {
  draft.value.enabled = (event.target as HTMLInputElement).checked;
}
</script>

<template>
  <article class="panel" data-test="alert-overview-card">
    <div class="overview-intro">
      <h2 class="card-title">{{ t('alerts.overview.title') }}</h2>
      <p class="note">{{ t('alerts.overview.note') }}</p>
    </div>
    <div class="tiles">
      <div v-for="tile in tiles" :key="tile.label" class="tile">
        <span class="tile__label">{{ tile.label }}</span>
        <strong class="tile__value">{{ tile.value }}</strong>
      </div>
    </div>
    <label class="toggle">
      <input
        type="checkbox"
        :checked="draft.enabled"
        data-test="alerts-enabled"
        @change="setEnabled"
      />
      <span>{{ t('alerts.overview.enabled') }}</span>
    </label>
  </article>
</template>

<style scoped>
.panel {
  display: grid;
  grid-template-columns: minmax(220px, 1.2fr) minmax(0, 2fr) minmax(180px, auto);
  gap: 18px;
  align-items: center;
  background: var(--bg-card);
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  padding: 16px;
}
.card-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.note {
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.5;
  margin: 6px 0 0;
}
.tiles {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  border-left: 1px solid var(--border-soft);
  border-right: 1px solid var(--border-soft);
}
.tile {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 4px 18px;
  border-right: 1px solid var(--border-soft);
}
.tile:last-child {
  border-right: 0;
}
.tile__label {
  color: var(--text-muted);
  font-size: 12px;
}
.tile__value {
  color: var(--text-primary);
  font-size: 14px;
  word-break: break-word;
}
.toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-secondary);
  justify-self: end;
  white-space: nowrap;
}
@media (max-width: 980px) {
  .panel {
    grid-template-columns: minmax(0, 1fr);
  }
  .tiles {
    border: 1px solid var(--border-soft);
    border-radius: 8px;
    overflow: hidden;
  }
  .tile {
    padding: 12px;
  }
  .toggle {
    justify-self: start;
  }
}
@media (max-width: 620px) {
  .tiles {
    grid-template-columns: minmax(0, 1fr);
  }
  .tile {
    border-right: 0;
    border-bottom: 1px solid var(--border-soft);
  }
  .tile:last-child {
    border-bottom: 0;
  }
}
</style>
