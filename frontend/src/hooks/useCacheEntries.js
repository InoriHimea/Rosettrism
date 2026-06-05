import { useCallback, useState } from 'react';

export function useCacheEntries(apiClient, t) {
  const [health, setHealth] = useState(null);
  const [stats, setStats] = useState(null);
  const [cache, setCache] = useState([]);
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
      const entries = cacheData.entries || [];
      setCache(entries);
      setSelectedCacheEntry((selected) => {
        if (!selected) {
          return selected;
        }
        return entries.find((entry) => entry.id === selected.id) || selected;
      });
    } catch {
      return;
    }
  }, [apiClient]);

  const deleteCache = useCallback(async (id) => {
    if (!window.confirm(t.deleteConfirm)) {
      return;
    }
    await apiClient.delete(`/api/cache/${id}`);
    if (selectedCacheEntry?.id === id) {
      setSelectedCacheEntry(null);
      setCacheDetail(null);
    }
    await refreshMeta();
  }, [apiClient, refreshMeta, selectedCacheEntry?.id, t.deleteConfirm]);

  const selectCacheEntry = useCallback(async (entry) => {
    setSelectedCacheEntry(entry);
    setCacheDetail(null);
    setCacheDetailBusy(true);
    try {
      const data = await apiClient.getJson(`/api/cache/${entry.id}`);
      setCacheDetail(data.entry || null);
    } catch (err) {
      setCacheDetail({ error: err.message });
    } finally {
      setCacheDetailBusy(false);
    }
  }, [apiClient]);

  return {
    health,
    stats,
    cache,
    selectedCacheEntry,
    cacheDetail,
    cacheDetailBusy,
    refreshMeta,
    selectCacheEntry,
    deleteCache,
  };
}
