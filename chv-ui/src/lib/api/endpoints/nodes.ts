import { get, post, del } from '@/lib/api/http';
import type { Node, NodeCreateRequest, ListResponse } from '@/types';

const BASE_PATH = '/api/v1/nodes';

export const nodesApi = {
  list: () => get<ListResponse<Node>>(BASE_PATH),
  
  get: (id: string) => get<Node>(`${BASE_PATH}/${id}`),
  
  register: (data: NodeCreateRequest) => 
    post<Node>(`${BASE_PATH}/register`, data),
  
  delete: (id: string) => del<void>(`${BASE_PATH}/${id}`),
  
  maintenance: (id: string, enabled: boolean) =>
    post<void>(`${BASE_PATH}/${id}/maintenance`, { enabled }),
};
