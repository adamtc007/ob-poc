/**
 * Inspector Store
 *
 * State management for the Inspector UI using Zustand.
 */

import { create } from 'zustand';
import type {
  InspectorProjection,
  ProjectionNode,
  LodTier,
  ProjectionPolicy,
} from '../types/projection';

/** Navigation history entry */
interface HistoryEntry {
  nodeId: string;
  label: string;
  timestamp: number;
}

/** Inspector state */
interface InspectorState {
  // Projection data
  projection: InspectorProjection | null;
  isLoading: boolean;
  error: string | null;

  // Navigation
  focusedNodeId: string | null;
  expandedNodes: Set<string>;
  selectedNodeId: string | null;

  // History (for back/forward)
  history: HistoryEntry[];
  historyIndex: number;

  // Policy controls
  policy: ProjectionPolicy;

  // Search
  searchQuery: string;
  searchResults: string[];
}

/** Inspector actions */
interface InspectorActions {
  // Projection loading
  setProjection: (projection: InspectorProjection) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;

  // Navigation
  focusNode: (nodeId: string, label: string) => void;
  goBack: () => void;
  goForward: () => void;
  canGoBack: () => boolean;
  canGoForward: () => boolean;

  // Tree expansion
  toggleExpanded: (nodeId: string) => void;
  expandNode: (nodeId: string) => void;
  collapseNode: (nodeId: string) => void;
  expandAll: () => void;
  collapseAll: () => void;

  // Selection
  selectNode: (nodeId: string | null) => void;

  // Policy
  setLod: (lod: LodTier) => void;
  setMaxDepth: (depth: number) => void;
  toggleChamber: (chamber: string) => void;

  // Search
  setSearchQuery: (query: string) => void;
  clearSearch: () => void;

  // Utils
  getNodeById: (nodeId: string) => ProjectionNode | null;
  getBreadcrumbs: () => HistoryEntry[];
  reset: () => void;
}

const DEFAULT_POLICY: ProjectionPolicy = {
  lod: 1,
  max_depth: 3,
  chambers: ['cbu', 'entity', 'trading'],
};

const initialState: InspectorState = {
  projection: null,
  isLoading: false,
  error: null,
  focusedNodeId: null,
  expandedNodes: new Set(),
  selectedNodeId: null,
  history: [],
  historyIndex: -1,
  policy: DEFAULT_POLICY,
  searchQuery: '',
  searchResults: [],
};

/** Find a node by ID in the projection tree */
function findNode(node: ProjectionNode, nodeId: string): ProjectionNode | null {
  if (node.id === nodeId) return node;
  if (node.children) {
    for (const child of node.children) {
      const found = findNode(child, nodeId);
      if (found) return found;
    }
  }
  return null;
}

/** Collect all node IDs in the tree */
function collectNodeIds(node: ProjectionNode, ids: Set<string> = new Set()): Set<string> {
  ids.add(node.id);
  if (node.children) {
    for (const child of node.children) {
      collectNodeIds(child, ids);
    }
  }
  return ids;
}

export const useInspectorStore = create<InspectorState & InspectorActions>((set, get) => ({
  ...initialState,

  setProjection: (projection) => {
    const rootId = projection.root.id;
    set({
      projection,
      focusedNodeId: rootId,
      expandedNodes: new Set([rootId]),
      selectedNodeId: null,
      history: [{ nodeId: rootId, label: projection.root.label, timestamp: Date.now() }],
      historyIndex: 0,
      error: null,
    });
  },

  setLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error, isLoading: false }),

  focusNode: (nodeId, label) => {
    const { history, historyIndex } = get();
    // Truncate forward history and add new entry
    const newHistory = history.slice(0, historyIndex + 1);
    newHistory.push({ nodeId, label, timestamp: Date.now() });

    set({
      focusedNodeId: nodeId,
      history: newHistory,
      historyIndex: newHistory.length - 1,
      expandedNodes: new Set([...get().expandedNodes, nodeId]),
    });
  },

  goBack: () => {
    const { history, historyIndex } = get();
    if (historyIndex > 0) {
      const newIndex = historyIndex - 1;
      set({
        historyIndex: newIndex,
        focusedNodeId: history[newIndex].nodeId,
      });
    }
  },

  goForward: () => {
    const { history, historyIndex } = get();
    if (historyIndex < history.length - 1) {
      const newIndex = historyIndex + 1;
      set({
        historyIndex: newIndex,
        focusedNodeId: history[newIndex].nodeId,
      });
    }
  },

  canGoBack: () => get().historyIndex > 0,
  canGoForward: () => get().historyIndex < get().history.length - 1,

  toggleExpanded: (nodeId) => {
    const { expandedNodes } = get();
    const newExpanded = new Set(expandedNodes);
    if (newExpanded.has(nodeId)) {
      newExpanded.delete(nodeId);
    } else {
      newExpanded.add(nodeId);
    }
    set({ expandedNodes: newExpanded });
  },

  expandNode: (nodeId) => {
    const { expandedNodes } = get();
    set({ expandedNodes: new Set([...expandedNodes, nodeId]) });
  },

  collapseNode: (nodeId) => {
    const { expandedNodes } = get();
    const newExpanded = new Set(expandedNodes);
    newExpanded.delete(nodeId);
    set({ expandedNodes: newExpanded });
  },

  expandAll: () => {
    const { projection } = get();
    if (projection) {
      const allIds = collectNodeIds(projection.root);
      set({ expandedNodes: allIds });
    }
  },

  collapseAll: () => {
    const { projection } = get();
    if (projection) {
      // Keep only root expanded
      set({ expandedNodes: new Set([projection.root.id]) });
    }
  },

  selectNode: (nodeId) => set({ selectedNodeId: nodeId }),

  setLod: (lod) => {
    const { policy } = get();
    set({ policy: { ...policy, lod } });
  },

  setMaxDepth: (max_depth) => {
    const { policy } = get();
    set({ policy: { ...policy, max_depth } });
  },

  toggleChamber: (chamber) => {
    const { policy } = get();
    const chambers = policy.chambers.includes(chamber)
      ? policy.chambers.filter(c => c !== chamber)
      : [...policy.chambers, chamber];
    set({ policy: { ...policy, chambers } });
  },

  setSearchQuery: (searchQuery) => {
    // TODO: Implement search with Fuse.js
    set({ searchQuery, searchResults: [] });
  },

  clearSearch: () => set({ searchQuery: '', searchResults: [] }),

  getNodeById: (nodeId) => {
    const { projection } = get();
    if (!projection) return null;
    return findNode(projection.root, nodeId);
  },

  getBreadcrumbs: () => {
    const { history, historyIndex } = get();
    return history.slice(0, historyIndex + 1);
  },

  reset: () => set(initialState),
}));

export default useInspectorStore;
