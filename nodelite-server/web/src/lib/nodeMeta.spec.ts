import { describe, expect, it } from 'vitest';
import { makeNodeStatus } from '@/api/__fixtures__/nodes';
import { effectiveGeoLocation, ipFromNode, locationFromNode } from './nodeMeta';

describe('locationFromNode', () => {
  it('reads a loc/region/city tag', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: ['region:eu-west'] },
    });
    expect(locationFromNode(node)).toBe('eu-west');
  });

  it('returns null when no location tag', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: ['env:prod'] },
    });
    expect(locationFromNode(node)).toBeNull();
  });

  it('falls back to geoip city and country', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: [] },
      geoip_city: 'Tokyo',
      geoip_country: 'JP',
    });
    expect(locationFromNode(node)).toBe('Tokyo, JP');
  });

  it('uses manual location before tags and geoip', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: ['region:us-west'] },
      geoip_city: 'Shenyang',
      geoip_country: 'CN',
      location_override_city: 'Hong Kong',
      location_override_country: 'HK',
    });
    expect(locationFromNode(node)).toBe('Hong Kong, HK');
  });

  it('does not mix automatic geoip fields into a partial manual location', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: [] },
      geoip_city: 'Shenyang',
      geoip_country: 'CN',
      geoip_latitude: 41.8057,
      geoip_longitude: 123.4315,
      location_override_country: '香港',
    });
    expect(locationFromNode(node)).toBe('香港');
    expect(effectiveGeoLocation(node)).toEqual({
      country: '香港',
      city: null,
      latitude: null,
      longitude: null,
      manual: true,
    });
  });

  it('reports LAN geoip without hostname inference', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: [] },
      geoip_country: 'LAN',
    });
    expect(locationFromNode(node)).toBe('LAN');
  });
});

describe('ipFromNode', () => {
  it('prefers an ip/addr tag', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: ['ip:10.0.0.5'] },
      remote_ip: '203.0.113.7',
    });
    expect(ipFromNode(node)).toBe('10.0.0.5');
  });

  it('falls back to remote_ip', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: [] },
      remote_ip: '203.0.113.7',
    });
    expect(ipFromNode(node)).toBe('203.0.113.7');
  });

  it('returns null when neither is present', () => {
    const node = makeNodeStatus({
      identity: { ...makeNodeStatus().identity, tags: [] },
      remote_ip: null,
    });
    expect(ipFromNode(node)).toBeNull();
  });
});
