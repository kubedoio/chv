<!-- #514: this component never renders. The route's +page.ts load
     function throws a 307 redirect to /volumes before any rendering
     happens — it is the live compatibility path for /storage
     bookmarks. The phantom storage-pools catalog UI (this page's
     former body + CreateStoragePoolModal + its BFF/API clients) was
     removed with the catalog: the storage_pools table was connected
     to no provisioning, placement, or stord configuration — an
     operator-invented fiction (see the 0058 migration). The truthful
     storage inventory is /volumes (volumes + backend class via the
     #379 storage-class surface). -->
