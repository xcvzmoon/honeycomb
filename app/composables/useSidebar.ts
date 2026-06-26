export function useSidebar() {
  const isSidebarCollapsed = useState<boolean>('is-sidebar-collapsed', () => {
    return localStorage.getItem('is-sidebar-collapsed') === 'true';
  });

  function toggleSidebar() {
    isSidebarCollapsed.value = !isSidebarCollapsed.value;
  }

  watch(isSidebarCollapsed, (value) => {
    localStorage.setItem('is-sidebar-collapsed', String(value));
  });

  return {
    isSidebarCollapsed: readonly(isSidebarCollapsed),
    toggleSidebar,
  };
}
