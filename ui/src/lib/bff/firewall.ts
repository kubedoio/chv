import { bffFetch } from './client';
import { BFFEndpoints } from './endpoints';

export interface FirewallRuleItem {
	id: string;
	network_id: string;
	direction: string;
	action: string;
	protocol: string;
	port_range: string;
	source_cidr: string;
	description: string;
	priority: number;
	created_at: string;
}

export interface CreateFirewallRuleInput {
	network_id: string;
	direction: string;
	action: string;
	protocol: string;
	port_range?: string;
	source_cidr?: string;
	description?: string;
	priority?: number;
}

export async function listFirewallRules(
	network_id: string,
	token?: string
): Promise<FirewallRuleItem[]> {
	return bffFetch(BFFEndpoints.listFirewallRules, {
		method: 'POST',
		body: JSON.stringify({ network_id }),
		token
	});
}

export async function createFirewallRule(
	data: CreateFirewallRuleInput,
	token?: string
): Promise<FirewallRuleItem> {
	return bffFetch(BFFEndpoints.createFirewallRule, {
		method: 'POST',
		body: JSON.stringify(data),
		token
	});
}

export async function deleteFirewallRule(
	rule_id: string,
	token?: string
): Promise<{ deleted: boolean; id: string }> {
	return bffFetch(BFFEndpoints.deleteFirewallRule, {
		method: 'POST',
		body: JSON.stringify({ rule_id }),
		token
	});
}
