import { goto } from '$app/navigation';
import {
  Terminal,
  Plus,
  RefreshCw,
  Settings,
  Image,
  Network,
  HardDrive,
  Command,
  Home,
  Server,
  Download
} from 'lucide-svelte';

export interface QuickAction {
  id: string;
  title: string;
  description: string;
  icon: typeof Terminal;
  shortcut?: string[];
  keywords: string[];
  section: string;
  action: () => void;
}

/**
 * Build the list of quick actions.
 *
 * `closePalette` is invoked by actions that should dismiss the palette
 * before performing their effect (e.g. opening the help dialog handled
 * by an external store).
 */
export function buildQuickActions(closePalette: () => void): QuickAction[] {
  return [
    {
      id: 'create-vm',
      title: 'Create Virtual Machine',
      description: 'Open virtual machine inventory',
      icon: Plus,
      shortcut: ['c'],
      keywords: ['vm', 'create', 'new', 'virtual machine', 'launch'],
      section: 'VMs',
      action: () => goto('/vms')
    },
    {
      id: 'go-dashboard',
      title: 'Go to Overview',
      description: 'View fleet overview',
      icon: Home,
      shortcut: ['g', 'd'],
      keywords: ['overview', 'dashboard', 'home', 'main'],
      section: 'Navigation',
      action: () => goto('/')
    },
    {
      id: 'go-vms',
      title: 'Go to Virtual Machines',
      description: 'View all VMs',
      icon: Server,
      shortcut: ['g', 'v'],
      keywords: ['vms', 'virtual machines', 'instances'],
      section: 'Navigation',
      action: () => goto('/vms')
    },
    {
      id: 'go-images',
      title: 'Go to Images',
      description: 'Manage OS images',
      icon: Image,
      shortcut: ['g', 'i'],
      keywords: ['images', 'os', 'templates', 'iso'],
      section: 'Navigation',
      action: () => goto('/images')
    },
    {
      id: 'go-volumes',
      title: 'Go to Volumes',
      description: 'View volume inventory',
      icon: HardDrive,
      shortcut: ['g', 's'],
      keywords: ['volumes', 'storage', 'pools', 'disks'],
      section: 'Navigation',
      action: () => goto('/volumes')
    },
    {
      id: 'go-networks',
      title: 'Go to Networks',
      description: 'Manage network configuration',
      icon: Network,
      shortcut: ['g', 'n'],
      keywords: ['networks', 'bridges', 'interfaces', 'vlan'],
      section: 'Navigation',
      action: () => goto('/networks')
    },
    {
      id: 'import-image',
      title: 'Import Image',
      description: 'Open images inventory',
      icon: Download,
      keywords: ['import', 'download', 'image', 'os'],
      section: 'Images',
      action: () => goto('/images')
    },
    {
      id: 'create-network',
      title: 'Open Networks',
      description: 'Open network inventory',
      icon: Network,
      keywords: ['network', 'create', 'bridge', 'vlan'],
      section: 'Networks',
      action: () => goto('/networks')
    },
    {
      id: 'refresh-data',
      title: 'Refresh All Data',
      description: 'Reload current page data',
      icon: RefreshCw,
      shortcut: ['r'],
      keywords: ['refresh', 'reload', 'update', 'sync'],
      section: 'System',
      action: () => window.location.reload()
    },
    {
      id: 'open-settings',
      title: 'Open Settings',
      description: 'System configuration',
      icon: Settings,
      keywords: ['settings', 'config', 'preferences'],
      section: 'System',
      action: () => goto('/settings')
    },
    {
      id: 'open-help',
      title: 'Keyboard Shortcuts Help',
      description: 'View all available shortcuts',
      icon: Command,
      shortcut: ['?'],
      keywords: ['help', 'shortcuts', 'keyboard', 'hotkeys'],
      section: 'System',
      action: () => {
        closePalette();
        // The keyboard store handles the '?' key
      }
    }
  ];
}
