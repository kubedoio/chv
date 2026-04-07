import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import InstallStatusPanel from '$lib/components/InstallStatusPanel.svelte';

describe('InstallStatusPanel', () => {
  it('renders core install state fields', () => {
    render(InstallStatusPanel, {
      status: {
        overall_state: 'ready',
        data_root: '/var/lib/chv',
        database_path: '/var/lib/chv/chv.db',
        bridge: {
          name: 'chvbr0',
          exists: true,
          expected_ip: '10.0.0.1/24',
          actual_ip: '10.0.0.1/24',
          up: true
        },
        localdisk: {
          path: '/var/lib/chv/storage/localdisk',
          ready: true
        },
        cloud_hypervisor: {
          path: '/usr/bin/cloud-hypervisor',
          found: true
        },
        cloudinit: {
          supported: true
        },
        checks: [],
        warnings: [],
        errors: []
      },
      handleBootstrap: vi.fn(),
      handleRefresh: vi.fn(),
      handleRepairBridge: vi.fn(),
      handleRepairDirectories: vi.fn(),
      handleRepairLocaldisk: vi.fn()
    });

    expect(screen.getByText('Bootstrap and Host Readiness')).toBeTruthy();
    expect(screen.getByText('/var/lib/chv/chv.db')).toBeTruthy();
    expect(screen.getByText('chvbr0')).toBeTruthy();
  });
});
