import { Store } from '@tanstack/react-store';
import type { TaskPriority } from '../types/kanban';

export interface WorkspaceState {
  activeWorkspaceId: string | null;
  searchQuery: string;
  selectedPriorities: TaskPriority[];
  selectedAssignees: string[];
  onlyMyTasks: boolean;
  viewMode: 'board' | 'table';
}

const STORAGE_KEY_WS = 'hb_kanban_active_workspace_id';

const initialWorkspaceState: WorkspaceState = {
  activeWorkspaceId: localStorage.getItem(STORAGE_KEY_WS) || null,
  searchQuery: '',
  selectedPriorities: [],
  selectedAssignees: [],
  onlyMyTasks: false,
  viewMode: 'board',
};

export const workspaceStore = new Store<WorkspaceState>(initialWorkspaceState);

export function setActiveWorkspaceId(id: string | null) {
  workspaceStore.setState((prev) => ({ ...prev, activeWorkspaceId: id }));
  if (id) {
    localStorage.setItem(STORAGE_KEY_WS, id);
  } else {
    localStorage.removeItem(STORAGE_KEY_WS);
  }
}

export function setSearchQuery(query: string) {
  workspaceStore.setState((prev) => ({ ...prev, searchQuery: query }));
}

export function togglePriorityFilter(priority: TaskPriority) {
  workspaceStore.setState((prev) => {
    const exists = prev.selectedPriorities.includes(priority);
    const updated = exists
      ? prev.selectedPriorities.filter((p) => p !== priority)
      : [...prev.selectedPriorities, priority];
    return { ...prev, selectedPriorities: updated };
  });
}

export function toggleAssigneeFilter(assigneeId: string) {
  workspaceStore.setState((prev) => {
    const exists = prev.selectedAssignees.includes(assigneeId);
    const updated = exists
      ? prev.selectedAssignees.filter((id) => id !== assigneeId)
      : [...prev.selectedAssignees, assigneeId];
    return { ...prev, selectedAssignees: updated };
  });
}

export function setOnlyMyTasks(only: boolean) {
  workspaceStore.setState((prev) => ({ ...prev, onlyMyTasks: only }));
}

export function setViewMode(mode: 'board' | 'table') {
  workspaceStore.setState((prev) => ({ ...prev, viewMode: mode }));
}

export function resetFilters() {
  workspaceStore.setState((prev) => ({
    ...prev,
    searchQuery: '',
    selectedPriorities: [],
    selectedAssignees: [],
    onlyMyTasks: false,
  }));
}
