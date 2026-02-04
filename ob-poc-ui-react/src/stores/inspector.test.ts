/**
 * Inspector Store Tests
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useInspectorStore } from './inspector';
import type { InspectorProjection } from '../types/projection';

const mockProjection: InspectorProjection = {
  version: '1.0',
  generated_at: '2024-01-01T00:00:00Z',
  policy: {
    lod: 1,
    max_depth: 3,
    chambers: ['cbu'],
  },
  root: {
    id: 'root-1',
    kind: 'cbu',
    label: 'Test CBU',
    meta: {
      chamber: 'cbu',
      lod_generated: 1,
    },
    fields: {
      name: 'Test CBU',
    },
    children: [
      {
        id: 'child-1',
        kind: 'entity',
        label: 'Child Entity',
        meta: {
          chamber: 'entity',
          lod_generated: 1,
        },
        fields: {},
      },
    ],
  },
};

describe('useInspectorStore', () => {
  beforeEach(() => {
    useInspectorStore.getState().reset();
  });

  describe('setProjection', () => {
    it('should set projection and initialize navigation state', () => {
      const store = useInspectorStore.getState();
      store.setProjection(mockProjection);

      const state = useInspectorStore.getState();
      expect(state.projection).toEqual(mockProjection);
      expect(state.focusedNodeId).toBe('root-1');
      expect(state.expandedNodes.has('root-1')).toBe(true);
      expect(state.history).toHaveLength(1);
      expect(state.historyIndex).toBe(0);
    });
  });

  describe('navigation', () => {
    beforeEach(() => {
      useInspectorStore.getState().setProjection(mockProjection);
    });

    it('should focus a node and update history', () => {
      const store = useInspectorStore.getState();
      store.focusNode('child-1', 'Child Entity');

      const state = useInspectorStore.getState();
      expect(state.focusedNodeId).toBe('child-1');
      expect(state.history).toHaveLength(2);
      expect(state.historyIndex).toBe(1);
    });

    it('should go back in history', () => {
      const store = useInspectorStore.getState();
      store.focusNode('child-1', 'Child Entity');
      store.goBack();

      const state = useInspectorStore.getState();
      expect(state.focusedNodeId).toBe('root-1');
      expect(state.historyIndex).toBe(0);
    });

    it('should go forward in history', () => {
      const store = useInspectorStore.getState();
      store.focusNode('child-1', 'Child Entity');
      store.goBack();
      store.goForward();

      const state = useInspectorStore.getState();
      expect(state.focusedNodeId).toBe('child-1');
      expect(state.historyIndex).toBe(1);
    });

    it('should report canGoBack and canGoForward correctly', () => {
      const store = useInspectorStore.getState();

      expect(store.canGoBack()).toBe(false);
      expect(store.canGoForward()).toBe(false);

      store.focusNode('child-1', 'Child Entity');
      expect(store.canGoBack()).toBe(true);
      expect(store.canGoForward()).toBe(false);

      store.goBack();
      expect(store.canGoBack()).toBe(false);
      expect(store.canGoForward()).toBe(true);
    });
  });

  describe('tree expansion', () => {
    beforeEach(() => {
      useInspectorStore.getState().setProjection(mockProjection);
    });

    it('should toggle node expansion', () => {
      const store = useInspectorStore.getState();
      expect(store.expandedNodes.has('root-1')).toBe(true);

      store.toggleExpanded('root-1');
      expect(useInspectorStore.getState().expandedNodes.has('root-1')).toBe(false);

      store.toggleExpanded('root-1');
      expect(useInspectorStore.getState().expandedNodes.has('root-1')).toBe(true);
    });

    it('should expand all nodes', () => {
      const store = useInspectorStore.getState();
      store.expandAll();

      const state = useInspectorStore.getState();
      expect(state.expandedNodes.has('root-1')).toBe(true);
      expect(state.expandedNodes.has('child-1')).toBe(true);
    });

    it('should collapse all but root', () => {
      const store = useInspectorStore.getState();
      store.expandAll();
      store.collapseAll();

      const state = useInspectorStore.getState();
      expect(state.expandedNodes.has('root-1')).toBe(true);
      expect(state.expandedNodes.size).toBe(1);
    });
  });

  describe('policy', () => {
    it('should update LOD', () => {
      const store = useInspectorStore.getState();
      store.setLod(2);

      expect(useInspectorStore.getState().policy.lod).toBe(2);
    });

    it('should update max depth', () => {
      const store = useInspectorStore.getState();
      store.setMaxDepth(5);

      expect(useInspectorStore.getState().policy.max_depth).toBe(5);
    });

    it('should toggle chambers', () => {
      const store = useInspectorStore.getState();

      store.toggleChamber('kyc');
      expect(useInspectorStore.getState().policy.chambers).toContain('kyc');

      store.toggleChamber('kyc');
      expect(useInspectorStore.getState().policy.chambers).not.toContain('kyc');
    });
  });

  describe('getNodeById', () => {
    beforeEach(() => {
      useInspectorStore.getState().setProjection(mockProjection);
    });

    it('should find root node', () => {
      const node = useInspectorStore.getState().getNodeById('root-1');
      expect(node).not.toBeNull();
      expect(node?.label).toBe('Test CBU');
    });

    it('should find child node', () => {
      const node = useInspectorStore.getState().getNodeById('child-1');
      expect(node).not.toBeNull();
      expect(node?.label).toBe('Child Entity');
    });

    it('should return null for non-existent node', () => {
      const node = useInspectorStore.getState().getNodeById('non-existent');
      expect(node).toBeNull();
    });
  });
});
