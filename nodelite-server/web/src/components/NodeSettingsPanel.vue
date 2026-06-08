<script setup lang="ts">
import { computed, onMounted, reactive, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import ReauthFields from '@/components/ReauthFields.vue';
import SettingsMessage from '@/components/SettingsMessage.vue';
import { apiClient } from '@/api';
import { ApiAbortError } from '@/api/client';
import { messageFromError } from '@/lib/apiError';
import { useSettingsStore } from '@/stores/settings';
import { fmtDateTime } from '@/lib/format';

/**
 * Per-node settings tab: shows the current node's token info (from the global
 * settings store's agents array) and a refresh-token form with reauth. The
 * server's POST /api/nodes/{id}/refresh-token returns the new expiry; on
 * success, reload the settings store so the token table reflects the change.
 */
const props = defineProps<{ nodeId: string }>();

const { t } = useI18n();
const settingsStore = useSettingsStore();

const reauth = reactive({ current_password: '', code: '' });
const message = reactive<{ state: 'ok' | 'error' | null; text: string }>({ state: null, text: '' });
const saving = reactive({ value: false });
const serviceDraft = reactive({ serviceDate: '', serviceUnlimited: false, renewalPrice: '' });
const serviceMessage = reactive<{ state: 'ok' | 'error' | null; text: string }>({
  state: null,
  text: '',
});
const serviceSaving = reactive({ value: false });
const locationDraft = reactive({ country: '', city: '', latitude: '', longitude: '' });
const locationMessage = reactive<{ state: 'ok' | 'error' | null; text: string }>({
  state: null,
  text: '',
});
const locationSaving = reactive({ value: false });

const agent = computed(() =>
  settingsStore.data?.agents.find((a) => a.node_id === props.nodeId),
);

onMounted(() => {
  if (!settingsStore.data && !settingsStore.loading) {
    void settingsStore.load();
  }
});

const expiryLabel = computed(() => {
  const a = agent.value;
  if (!a) return '—';
  if (!a.token_expires_at) return t('node.settings.token_never_expires');
  const secs = a.token_expires_in_secs;
  if (secs == null || secs < 0) return t('node.settings.token_expired');
  const days = Math.floor(secs / 86400);
  if (days > 0) return t('node.settings.token_expires_in_days', { days });
  const hours = Math.floor(secs / 3600);
  return t('node.settings.token_expires_in_hours', { hours });
});

const expiryDate = computed(() => {
  const a = agent.value;
  return a?.token_expires_at ? fmtDateTime(a.token_expires_at) : null;
});

const automaticLocation = computed(() => {
  const a = agent.value;
  if (!a) return '—';
  const parts = [a.geoip_city, a.geoip_country].filter(Boolean);
  if (parts.length > 0) return parts.join(', ');
  if (a.geoip_latitude != null && a.geoip_longitude != null) {
    return `${a.geoip_latitude.toFixed(4)}, ${a.geoip_longitude.toFixed(4)}`;
  }
  return '—';
});

function dateInputValue(value: string | null | undefined): string {
  if (!value) return '';
  const ms = Date.parse(value);
  if (Number.isFinite(ms)) return new Date(ms).toISOString().slice(0, 10);
  return /^\d{4}-\d{2}-\d{2}/.test(value) ? value.slice(0, 10) : '';
}

function serviceExpiresAt(value: string): string | null {
  return value ? `${value}T00:00:00Z` : null;
}

watch(
  agent,
  (value) => {
    serviceDraft.serviceDate = dateInputValue(value?.service_expires_at);
    serviceDraft.serviceUnlimited = value?.service_unlimited ?? false;
    serviceDraft.renewalPrice = value?.renewal_price ?? '';
    locationDraft.country = value?.location_override_country ?? '';
    locationDraft.city = value?.location_override_city ?? '';
    locationDraft.latitude =
      value?.location_override_latitude == null ? '' : String(value.location_override_latitude);
    locationDraft.longitude =
      value?.location_override_longitude == null ? '' : String(value.location_override_longitude);
  },
  { immediate: true },
);

function optionalNumber(value: string | number): number | null | undefined {
  const trimmed = String(value).trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

async function saveServiceMetadata(): Promise<void> {
  serviceMessage.state = null;
  serviceMessage.text = '';
  serviceSaving.value = true;
  try {
    const renewalPrice = serviceDraft.renewalPrice.trim();
    const resp = await apiClient.updateNodeServiceMetadata(props.nodeId, {
      service_expires_at: serviceDraft.serviceUnlimited
        ? null
        : serviceExpiresAt(serviceDraft.serviceDate),
      service_unlimited: serviceDraft.serviceUnlimited,
      renewal_price: renewalPrice || null,
    });
    await settingsStore.refresh();
    serviceDraft.renewalPrice = renewalPrice;
    serviceMessage.state = 'ok';
    serviceMessage.text = resp.message || t('node.settings.service_meta_saved');
  } catch (e) {
    if (e instanceof ApiAbortError) return;
    serviceMessage.state = 'error';
    serviceMessage.text = t('node.settings.service_meta_failed', {
      error: messageFromError(e, 'unknown'),
    });
  } finally {
    serviceSaving.value = false;
  }
}

async function saveLocationOverride(clear = false): Promise<void> {
  locationMessage.state = null;
  locationMessage.text = '';
  const latitude = clear ? null : optionalNumber(locationDraft.latitude);
  const longitude = clear ? null : optionalNumber(locationDraft.longitude);
  if (latitude === undefined || longitude === undefined) {
    locationMessage.state = 'error';
    locationMessage.text = t('node.settings.location_invalid_number');
    return;
  }

  locationSaving.value = true;
  try {
    const country = clear ? '' : locationDraft.country.trim();
    const city = clear ? '' : locationDraft.city.trim();
    const resp = await apiClient.updateNodeLocationOverride(props.nodeId, {
      country: country || null,
      city: city || null,
      latitude,
      longitude,
    });
    await settingsStore.refresh();
    locationDraft.country = country;
    locationDraft.city = city;
    if (clear) {
      locationDraft.latitude = '';
      locationDraft.longitude = '';
    }
    locationMessage.state = 'ok';
    locationMessage.text = resp.message || t('node.settings.location_saved');
  } catch (e) {
    if (e instanceof ApiAbortError) return;
    locationMessage.state = 'error';
    locationMessage.text = t('node.settings.location_failed', {
      error: messageFromError(e, 'unknown'),
    });
  } finally {
    locationSaving.value = false;
  }
}

async function refresh(): Promise<void> {
  message.state = null;
  message.text = t('node.settings.refreshing');
  saving.value = true;
  try {
    const body: { current_password?: string; code?: string } = {};
    if (reauth.current_password) body.current_password = reauth.current_password;
    if (reauth.code) body.code = reauth.code;
    const resp = await apiClient.refreshNodeToken(props.nodeId, body);
    await settingsStore.refresh();
    reauth.current_password = '';
    reauth.code = '';
    message.state = 'ok';
    message.text = resp.message || t('node.settings.token_refreshed');
  } catch (e) {
    if (e instanceof ApiAbortError) return;
    message.state = 'error';
    message.text = t('node.settings.refresh_failed', { error: messageFromError(e, 'unknown') });
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="node-settings" data-test="node-settings-panel">
    <article class="panel" data-test="node-token-info-panel">
      <header class="card-head">
        <h2 class="card-title">{{ t('node.settings.token_info') }}</h2>
      </header>

      <template v-if="agent">
        <div class="info-grid">
          <div class="info-row">
            <span class="info-label">{{ t('node.settings.token_status') }}</span>
            <span class="info-value">{{ expiryLabel }}</span>
          </div>
          <div v-if="expiryDate" class="info-row">
            <span class="info-label">{{ t('node.settings.token_expires_at') }}</span>
            <span class="info-value">{{ expiryDate }}</span>
          </div>
        </div>
      </template>
      <p v-else class="placeholder">
        {{ t('common.waiting_for_data') }}
      </p>
    </article>

    <article class="panel">
      <header class="card-head">
        <h2 class="card-title">{{ t('node.settings.service_meta') }}</h2>
      </header>

      <template v-if="agent">
        <div class="service-form">
          <label class="field">
            <span>{{ t('node.settings.service_expires_at') }}</span>
            <input
              v-model="serviceDraft.serviceDate"
              class="field-input"
              type="date"
              :disabled="serviceDraft.serviceUnlimited"
              data-test="node-service-expiry-input"
            />
          </label>
          <label class="field field--check">
            <span>{{ t('node.settings.service_unlimited') }}</span>
            <span class="check-row">
              <input
                v-model="serviceDraft.serviceUnlimited"
                type="checkbox"
                data-test="node-service-unlimited-input"
              />
              <span>{{ t('node.settings.service_unlimited_hint') }}</span>
            </span>
          </label>
          <label class="field">
            <span>{{ t('node.settings.renewal_price') }}</span>
            <input
              v-model="serviceDraft.renewalPrice"
              class="field-input"
              type="text"
              maxlength="64"
              :placeholder="t('settings.tokens.renewal_price_placeholder')"
              data-test="node-renewal-price-input"
            />
          </label>
          <button
            type="button"
            class="btn btn--primary service-save"
            :disabled="serviceSaving.value"
            data-test="node-service-meta-save"
            @click="saveServiceMetadata"
          >
            {{
              serviceSaving.value
                ? t('node.settings.service_meta_saving')
                : t('node.settings.service_meta_save')
            }}
          </button>
        </div>
        <SettingsMessage :state="serviceMessage.state" :text="serviceMessage.text" />
      </template>
      <p v-else class="placeholder">
        {{ t('common.waiting_for_data') }}
      </p>
    </article>

    <article class="panel">
      <header class="card-head">
        <h2 class="card-title">{{ t('node.settings.location_override') }}</h2>
      </header>

      <template v-if="agent">
        <div class="info-grid location-current">
          <div class="info-row">
            <span class="info-label">{{ t('node.settings.location_auto') }}</span>
            <span class="info-value">{{ automaticLocation }}</span>
          </div>
        </div>
        <div class="location-form">
          <label class="field">
            <span>{{ t('node.settings.location_country') }}</span>
            <input
              v-model="locationDraft.country"
              class="field-input"
              type="text"
              maxlength="64"
              placeholder="HK"
              data-test="node-location-country-input"
            />
          </label>
          <label class="field">
            <span>{{ t('node.settings.location_city') }}</span>
            <input
              v-model="locationDraft.city"
              class="field-input"
              type="text"
              maxlength="64"
              placeholder="Hong Kong"
              data-test="node-location-city-input"
            />
          </label>
          <label class="field">
            <span>{{ t('node.settings.location_latitude') }}</span>
            <input
              v-model="locationDraft.latitude"
              class="field-input"
              type="number"
              step="0.000001"
              min="-90"
              max="90"
              placeholder="22.3193"
              data-test="node-location-latitude-input"
            />
          </label>
          <label class="field">
            <span>{{ t('node.settings.location_longitude') }}</span>
            <input
              v-model="locationDraft.longitude"
              class="field-input"
              type="number"
              step="0.000001"
              min="-180"
              max="180"
              placeholder="114.1694"
              data-test="node-location-longitude-input"
            />
          </label>
          <div class="location-actions">
            <button
              type="button"
              class="btn btn--primary"
              :disabled="locationSaving.value"
              data-test="node-location-save"
              @click="saveLocationOverride()"
            >
              {{
                locationSaving.value
                  ? t('node.settings.location_saving')
                  : t('node.settings.location_save')
              }}
            </button>
            <button
              type="button"
              class="btn"
              :disabled="locationSaving.value"
              data-test="node-location-clear"
              @click="saveLocationOverride(true)"
            >
              {{ t('node.settings.location_clear') }}
            </button>
          </div>
        </div>
        <SettingsMessage :state="locationMessage.state" :text="locationMessage.text" />
      </template>
      <p v-else class="placeholder">
        {{ t('common.waiting_for_data') }}
      </p>
    </article>

    <article class="panel">
      <header class="card-head">
        <h2 class="card-title">{{ t('node.settings.refresh_token') }}</h2>
        <p class="card-note">{{ t('node.settings.refresh_note') }}</p>
      </header>

      <div class="refresh-form">
        <ReauthFields
          v-model:current-password="reauth.current_password"
          v-model:code="reauth.code"
          variant="both"
        />
        <button
          type="button"
          class="btn btn--primary"
          :disabled="saving.value"
          data-test="refresh-token-button"
          @click="refresh"
        >
          {{ t('node.settings.refresh_button') }}
        </button>
        <SettingsMessage :state="message.state" :text="message.text" />
      </div>
    </article>
  </div>
</template>

<style scoped>
.node-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.panel {
  background: var(--bg-card);
  border: 1px solid var(--border-soft);
  border-radius: 16px;
  padding: 18px 20px;
}
.card-head {
  margin-bottom: 14px;
}
.card-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.card-note {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 12px;
}
.info-grid {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  background: var(--bg-card-soft);
  border: 1px solid var(--border-soft);
  border-radius: 10px;
}
.info-label {
  font-size: 13px;
  color: var(--text-muted);
}
.info-value {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}
.placeholder {
  margin: 0;
  color: var(--text-muted);
  font-size: 13px;
}
.refresh-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.service-form {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(140px, 0.65fr) minmax(0, 1fr) auto;
  gap: 12px;
  align-items: end;
}
.location-current {
  margin-bottom: 12px;
}
.location-form {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr)) auto;
  gap: 12px;
  align-items: end;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  color: var(--text-muted);
  font-size: 12px;
}
.field-input {
  width: 100%;
  height: 36px;
  color: var(--text-primary);
  background: var(--bg-card-soft);
  border: 1px solid var(--border-soft);
  border-radius: 10px;
  padding: 0 10px;
  font: inherit;
  font-size: 13px;
}
.field-input:focus {
  border-color: var(--border-strong);
  outline: none;
}
.field-input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.field--check {
  color: var(--text-muted);
}
.check-row {
  display: inline-flex;
  align-items: center;
  min-height: 36px;
  gap: 8px;
  color: var(--text-secondary);
  font-size: 13px;
}
.check-row input {
  width: 15px;
  height: 15px;
  accent-color: var(--accent-green);
}
.service-save {
  min-height: 36px;
}
.location-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.btn {
  align-self: flex-start;
  background: var(--bg-card-soft);
  color: var(--text-secondary);
  border: 1px solid var(--border-soft);
  border-radius: 10px;
  padding: 8px 14px;
  font: inherit;
}
.btn--primary {
  color: #fff;
  background: var(--accent-blue);
  border-color: transparent;
}
.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
@media (max-width: 720px) {
  .service-form {
    grid-template-columns: 1fr;
  }
  .location-form {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .location-actions {
    justify-content: flex-start;
  }
}
@media (max-width: 560px) {
  .location-form {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
