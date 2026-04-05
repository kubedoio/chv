import { get, post } from '@/lib/api/http';
import type { Image, ImageImportRequest, ListResponse } from '@/types';

const BASE_PATH = '/api/v1/images';

export const imagesApi = {
  list: () => get<ListResponse<Image>>(BASE_PATH),
  
  get: (id: string) => get<Image>(`${BASE_PATH}/${id}`),
  
  import: (data: ImageImportRequest) => 
    post<Image>(`${BASE_PATH}/import`, data),
};
