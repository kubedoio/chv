import { get, post } from '@/lib/api/http';
import type { StoragePool, StoragePoolCreateRequest, ListResponse } from '@/types';

const BASE_PATH = '/api/v1/storage-pools';

export const storagePoolsApi = {
  list: () => get<ListResponse<StoragePool>>(BASE_PATH),
  
  get: (id: string) => get<StoragePool>(`${BASE_PATH}/${id}`),
  
  create: (data: StoragePoolCreateRequest) => post<StoragePool>(BASE_PATH, data),
};
