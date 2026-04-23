-- KVM capability tracking for node inventory

ALTER TABLE node_inventory ADD COLUMN hypervisor_capabilities text;
