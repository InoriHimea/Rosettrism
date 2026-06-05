import { useEffect, useState } from 'react';

export function useSidebarState() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(localStorage.getItem('rosettrism-sidebar') === 'collapsed');

  useEffect(() => {
    localStorage.setItem('rosettrism-sidebar', sidebarCollapsed ? 'collapsed' : 'expanded');
  }, [sidebarCollapsed]);

  return { sidebarCollapsed, setSidebarCollapsed };
}
