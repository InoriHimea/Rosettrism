import { useCallback, useState } from 'react';

const upstreamType = 'upstream';
const unifiedType = 'unified';

export function useCacheEntries(apiClient, t) {
  const [health, setHealth] = useState(null);
  const [stats, setStats] = useState(null);
  const [cache, setCache] = useState([]);
  const [unifiedCache, setUnifiedCache] = useState([]);
  const [selectedCacheEntry, setSelectedCacheEntry] = useState(null);
  const [cacheDetail, setCacheDetail] = useState(null);
  const [cacheDetailBusy, setCacheDetailBusy] = useState(false);

  const refreshMeta = useCallback(async () => {
    try {
      const [healthData, statsData, cacheData] = await Promise.all([
        apiClient.getJson('/api/health'),
        apiClient.getJson('/api/stats'),
        apiClient.getJson('/api/cache'),
      ]);
      setHealth(healthData);
      setStats(statsData);
      const upstreamEntries = (cacheData.upstream_entries || cacheData.entries || [])
        .map((entry) => ({ ...entry, cache_type: upstreamType }));
      const unifiedEntries = (cacheData.unified_entries || [])
        .map((entry) => ({ ...entry, cache_type: unifiedType }));
      setCache(upstreamEntries);
      setUnifiedCache(unifiedEntries);
      setSelectedCacheEntry((selected) => {
        if (!selected) {
          return selected;
        }
        const entries = selected.cache_type === unifiedType ? unifiedEntries : upstreamEntries;
        return entries.find((entry) => entry.id === selected.id) || selected;
      });
    } catch {
      return;
    }
  }, [apiClient]);

  const deleteCache = useCallback(async (entryOrId, cacheType = upstreamType) => {
    const id = typeof entryOrId === 'object' ? entryOrId.id : entryOrId;
    const type = typeof entryOrId === 'object' ? entryOrId.cache_type || cacheType : cacheType;
    if (!window.confirm(t.deleteConfirm)) {
      return;
    }
    const endpoint = type === unifiedType ? `/api/unified-cache/${id}` : `/api/cache/${id}`;
    await apiClient.delete(endpoint);
    if (selectedCacheEntry?.id === id && (selectedCacheEntry.cache_type || upstreamType) === type) {
      setSelectedCacheEntry(null);
      setCacheDetail(null);
    }
    await refreshMeta();
  }, [apiClient, refreshMeta, selectedCacheEntry, t.deleteConfirm]);

  const selectCacheEntry = useCallback(async (entry) => {
    const type = entry.cache_type || upstreamType;
    setSelectedCacheEntry(entry);
    setCacheDetail(null);
    setCacheDetailBusy(true);
    try {
      const endpoint = type === unifiedType ? `/api/unified-cache/${entry.id}` : `/api/cache/${entry.id}`;
      const data = await apiClient.getJson(endpoint);
      setCacheDetail(data);
    } catch (err) {
      setCacheDetail({ error: err });
    } finally {
      setCacheDetailBusy(false);
    }
  }, [apiClient]);

  return {
    health,
    stats,
    cache,
    unifiedCache,
    selectedCacheEntry,
    cacheDetail,
    cacheDetailBusy,
    refreshMeta,
    selectCacheEntry,
    deleteCache,
  };
}
