<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { SettingsAgentToken } from '@/api';
import { tokenRemaining, tokenSeverity } from '@/lib/format';

const props = defineProps<{ agents: SettingsAgentToken[] }>();
const { t, locale } = useI18n();

function fmtDateTime(value: string | null): string {
  if (!value) return t('settings.token.no_expiry');
  const ms = Date.parse(value);
  return Number.isFinite(ms) ? new Date(ms).toLocaleString(locale.value) : value;
}

function remainingText(seconds: number | null): string {
  const r = tokenRemaining(seconds);
  switch (r.kind) {
    case 'none':
      return t('settings.token.no_expiry');
    case 'expired':
      return t('settings.token.expired');
    case 'days_hours':
      return t('settings.duration.days_hours', { days: r.days, hours: r.hours });
    case 'minutes':
      return t('settings.duration.minutes', { minutes: r.minutes });
  }
}

const rows = computed(() =>
  props.agents.map((a) => ({
    id: a.node_id,
    label: a.node_label || a.node_id,
    nodeId: a.node_id,
    status: a.online ? t('common.online') : t('common.offline'),
    online: a.online,
    agent: a.agent_version ?? t('common.not_available'),
    ip: a.remote_ip ?? t('common.not_available'),
    expiresAt: fmtDateTime(a.token_expires_at),
    remaining: remainingText(a.token_expires_in_secs),
    severity: tokenSeverity(a.token_expires_in_secs),
  })),
);
</script>

<template>
  <article class="panel" data-test="token-table">
    <header class="card-head">
      <div>
        <span class="card-kicker">{{ t('settings.summary.token_health') }}</span>
        <h2 class="card-title">{{ t('settings.tokens.title') }}</h2>
      </div>
      <strong class="agent-count">{{ agents.length }}</strong>
    </header>
    <p v-if="rows.length === 0" class="empty" data-test="token-table-empty">
      {{ t('settings.tokens.empty') }}
    </p>
    <table v-else class="tokens">
      <thead>
        <tr>
          <th>{{ t('settings.tokens.node') }}</th>
          <th>{{ t('settings.tokens.status') }}</th>
          <th>{{ t('settings.tokens.agent') }}</th>
          <th>{{ t('settings.tokens.ip') }}</th>
          <th>{{ t('settings.tokens.expires_at') }}</th>
          <th class="numeric">{{ t('settings.tokens.remaining') }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.id" data-test="token-row">
          <td :data-label="t('settings.tokens.node')">
            {{ row.label }}<div class="subnote">{{ row.nodeId }}</div>
          </td>
          <td :data-label="t('settings.tokens.status')">
            <span class="status-pill" :class="row.online ? 'online' : 'offline'">
              {{ row.status }}
            </span>
          </td>
          <td :data-label="t('settings.tokens.agent')">{{ row.agent }}</td>
          <td :data-label="t('settings.tokens.ip')">{{ row.ip }}</td>
          <td :data-label="t('settings.tokens.expires_at')">{{ row.expiresAt }}</td>
          <td :data-label="t('settings.tokens.remaining')" class="numeric" :class="row.severity">
            {{ row.remaining }}
          </td>
        </tr>
      </tbody>
    </table>
  </article>
</template>

<style scoped>
.panel {
  background: var(--bg-card);
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  padding: 16px;
}
.card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}
.card-kicker {
  display: block;
  color: var(--text-muted);
  font-size: 12px;
  margin-bottom: 4px;
}
.card-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.agent-count {
  color: var(--text-primary);
  font-size: 22px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.empty {
  color: var(--text-muted);
  font-size: 13px;
  margin: 0;
}
.tokens {
  width: 100%;
  overflow: hidden;
  border: 1px solid var(--border-soft);
  border-radius: 8px;
  border-collapse: collapse;
  font-size: 13px;
}
.tokens th,
.tokens td {
  text-align: left;
  padding: 8px 10px;
  border-bottom: 1px solid var(--border-soft);
  vertical-align: top;
}
.tokens th {
  color: var(--text-muted);
  font-weight: 500;
  background: var(--bg-card-soft);
}
.tokens .numeric {
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.subnote {
  color: var(--text-muted);
  font-size: 11px;
}
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-weight: 600;
}
.status-pill::before {
  content: '';
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}
.status-pill.online {
  color: var(--accent-green);
}
.status-pill.offline {
  color: var(--chart-network-up);
}
.expired {
  color: var(--accent-red);
}
.expiring {
  color: var(--accent-yellow);
}
.ok {
  color: var(--accent-green);
}
@media (max-width: 640px) {
  .tokens,
  .tokens thead,
  .tokens tbody,
  .tokens tr,
  .tokens th,
  .tokens td {
    display: block;
  }
  .tokens thead {
    display: none;
  }
  .tokens tr {
    border-bottom: 1px solid var(--border-soft);
    padding: 10px 0;
  }
  .tokens tr:last-child {
    border-bottom: 0;
  }
  .tokens td {
    border-bottom: 0;
    display: grid;
    grid-template-columns: minmax(86px, 0.42fr) minmax(0, 1fr);
    gap: 10px;
    padding: 5px 0;
    overflow-wrap: anywhere;
  }
  .tokens td::before {
    content: attr(data-label);
    color: var(--text-muted);
  }
  .tokens .numeric {
    text-align: left;
  }
}
</style>
