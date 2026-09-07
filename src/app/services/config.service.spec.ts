import { describe, expect, it } from 'vitest';
import { DEFAULT_CONFIG } from '../models/config';

describe('generated defaults', () => {
  it('carry the values the Rust side defaults to', () => {
    expect(DEFAULT_CONFIG.theme).toBe('system');
    expect(DEFAULT_CONFIG.notifications.enabled).toBe(true);
    expect(DEFAULT_CONFIG.notifications.urgent_clips).toBe(false);
    expect(DEFAULT_CONFIG.processes.scan_interval_secs).toBe(3);
  });
});
