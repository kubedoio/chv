import { get, post, del } from '@/lib/api/http';
import type { VM, VMCreateRequest, ListResponse } from '@/types';

const BASE_PATH = '/api/v1/vms';

export const vmsApi = {
  list: () => get<ListResponse<VM>>(BASE_PATH),
  
  get: (id: string) => get<VM>(`${BASE_PATH}/${id}`),
  
  create: (data: VMCreateRequest) => post<VM>(BASE_PATH, data),
  
  delete: (id: string) => del<void>(`${BASE_PATH}/${id}`),
  
  start: (id: string) => post<void>(`${BASE_PATH}/${id}/start`, {}),
  
  stop: (id: string) => post<void>(`${BASE_PATH}/${id}/stop`, {}),
  
  reboot: (id: string) => post<void>(`${BASE_PATH}/${id}/reboot`, {}),
  
  resizeDisk: (id: string, volumeId: string, newSizeBytes: number) =>
    post<void>(`${BASE_PATH}/${id}/resize-disk`, { volume_id: volumeId, new_size_bytes: newSizeBytes }),
  
  console: (id: string) => `${BASE_PATH}/${id}/console`,
};
