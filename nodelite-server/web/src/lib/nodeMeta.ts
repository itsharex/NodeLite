/**
 * Pure tag parsers for node identity metadata, ported from
 * assets/node.html:1694-1709. Nodes encode IP/location as `key:value` tags.
 */

import type { NodeListItem, NodeStatus } from '@/api';

type LocationNode = Pick<NodeStatus | NodeListItem, 'identity'> & {
  geoip_country: string | null;
  geoip_city: string | null;
  geoip_latitude: number | null;
  geoip_longitude: number | null;
  location_override_country: string | null;
  location_override_city: string | null;
  location_override_latitude: number | null;
  location_override_longitude: number | null;
};

export interface EffectiveGeoLocation {
  country: string | null;
  city: string | null;
  latitude: number | null;
  longitude: number | null;
  manual: boolean;
}

export function effectiveGeoLocation(node: LocationNode): EffectiveGeoLocation {
  const manual = Boolean(
    node.location_override_country ||
      node.location_override_city ||
      node.location_override_latitude != null ||
      node.location_override_longitude != null,
  );
  if (manual) {
    return {
      country: node.location_override_country,
      city: node.location_override_city,
      latitude: node.location_override_latitude,
      longitude: node.location_override_longitude,
      manual: true,
    };
  }
  return {
    country: node.geoip_country,
    city: node.geoip_city,
    latitude: node.geoip_latitude,
    longitude: node.geoip_longitude,
    manual: false,
  };
}

/** Location from a manual override, `loc|location|region|city:value` tag, or GeoIP. */
export function locationFromNode(node: LocationNode): string | null {
  const geo = effectiveGeoLocation(node);
  if (geo.manual) {
    if (geo.country === 'LAN') return 'LAN';
    const parts = [geo.city, geo.country].filter(Boolean);
    return parts.length > 0 ? parts.join(', ') : null;
  }
  for (const tag of node.identity.tags || []) {
    const m = String(tag).match(/^(?:loc|location|region|city)[:=](.+)$/i);
    if (m && m[1]) return m[1];
  }
  if (geo.country === 'LAN') return 'LAN';
  if (geo.city && geo.country) return `${geo.city}, ${geo.country}`;
  if (geo.country) return geo.country;
  return null;
}

/** IP from an `ip|addr|address:value` tag, falling back to remote_ip. */
export function ipFromNode(node: NodeStatus): string | null {
  for (const tag of node.identity.tags || []) {
    const m = String(tag).match(/^(?:ip|addr|address)[:=](.+)$/i);
    if (m && m[1]) return m[1];
  }
  return node.remote_ip || null;
}
