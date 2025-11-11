'use client';

import React, { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Search, Settings, Activity, Database } from 'lucide-react';

// Mock API functions (replace with actual API calls)
const api = {
  searchEntities: async (params: any) => {
    // Mock data
    return {
      data: [
        {
          id: '1',
          entity_type: 'partnership',
          name: 'TechCorp Solutions LLC',
          data: { jurisdiction: 'US-DE', partnership_type: 'Limited Liability' },
          created_at: '2024-01-15T10:00:00Z',
          updated_at: '2024-01-15T10:00:00Z'
        },
        {
          id: '2',
          entity_type: 'limited_company',
          name: 'AlphaTech Ltd',
          data: { jurisdiction: 'GB', registration_number: '12345678' },
          created_at: '2024-01-16T10:00:00Z',
          updated_at: '2024-01-16T10:00:00Z'
        }
      ]
    };
  },

  createEntity: async (entity: any) => {
    // Mock creation
    return {
      id: Math.random().toString(),
      ...entity,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString()
    };
  },

  generateDsl: async (instruction: string) => {
    // Mock DSL generation
    return {
      dsl_content: `(data.create :asset "partnership" :values {:name "${instruction}"})`,
      confidence: 0.95,
      provider_used: 'openai',
      explanation: 'Generated DSL from natural language instruction'
    };
  }
};

interface Entity {
  id: string;
  entity_type: string;
  name: string;
  data: Record<string, any>;
  created_at: string;
  updated_at: string;
}

export default function HomePage() {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedEntityType, setSelectedEntityType] = useState('all');
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [naturalLanguageInput, setNaturalLanguageInput] = useState('');
  const [generatedDsl, setGeneratedDsl] = useState('');

  const queryClient = useQueryClient();

  // Fetch entities
  const { data: entitiesResponse, isLoading, error } = useQuery({
    queryKey: ['entities', searchQuery, selectedEntityType],
    queryFn: () => api.searchEntities({
      name_contains: searchQuery,
      entity_type: selectedEntityType === 'all' ? undefined : selectedEntityType
    }),
  });

  // Generate DSL mutation
  const generateDslMutation = useMutation({
    mutationFn: api.generateDsl,
    onSuccess: (data) => {
      setGeneratedDsl(data.dsl_content);
    }
  });

  // Create entity mutation
  const createEntityMutation = useMutation({
    mutationFn: api.createEntity,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['entities'] });
      setShowCreateModal(false);
      setNaturalLanguageInput('');
      setGeneratedDsl('');
    }
  });

  const entities = entitiesResponse?.data || [];

  const handleGenerateDsl = async () => {
    if (naturalLanguageInput.trim()) {
      generateDslMutation.mutate(naturalLanguageInput);
    }
  };

  const handleCreateEntity = () => {
    // Parse the generated DSL and create entity
    // This is simplified - in production would parse DSL properly
    const entityName = naturalLanguageInput.split(' ').slice(-1)[0] || 'New Entity';

    createEntityMutation.mutate({
      entity_type: 'partnership',
      name: entityName,
      instruction: naturalLanguageInput,
      data: { generated_via: 'ai' }
    });
  };

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white shadow-sm border-b">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between items-center py-6">
            <div className="flex items-center">
              <Database className="h-8 w-8 text-blue-600 mr-3" />
              <div>
                <h1 className="text-2xl font-bold text-gray-900">OB-POC Entity Manager</h1>
                <p className="text-sm text-gray-600">Agentic CRUD for Financial Entities</p>
              </div>
            </div>
            <div className="flex items-center space-x-4">
              <button className="flex items-center px-3 py-2 text-sm text-gray-600 hover:text-gray-900">
                <Activity className="h-4 w-4 mr-2" />
                Monitor
              </button>
              <button className="flex items-center px-3 py-2 text-sm text-gray-600 hover:text-gray-900">
                <Settings className="h-4 w-4 mr-2" />
                Settings
              </button>
            </div>
          </div>
        </div>
      </header>

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Actions Bar */}
        <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center mb-8 gap-4">
          <div className="flex items-center space-x-4">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
              <input
                type="text"
                placeholder="Search entities..."
                className="pl-10 pr-4 py-2 w-64 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
            <select
              className="px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
              value={selectedEntityType}
              onChange={(e) => setSelectedEntityType(e.target.value)}
            >
              <option value="all">All Types</option>
              <option value="partnership">Partnerships</option>
              <option value="limited_company">Limited Companies</option>
              <option value="proper_person">Individuals</option>
              <option value="trust">Trusts</option>
            </select>
          </div>

          <button
            onClick={() => setShowCreateModal(true)}
            className="flex items-center px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 focus:ring-2 focus:ring-blue-500 focus:ring-offset-2"
          >
            <Plus className="h-4 w-4 mr-2" />
            Create Entity
          </button>
        </div>

        {/* Stats Cards */}
        <div className="grid grid-cols-1 md:grid-cols-4 gap-6 mb-8">
          <div className="bg-white p-6 rounded-lg shadow-sm">
            <div className="flex items-center">
              <div className="p-2 bg-blue-100 rounded-lg">
                <Database className="h-6 w-6 text-blue-600" />
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-gray-600">Total Entities</p>
                <p className="text-2xl font-semibold text-gray-900">{entities.length}</p>
              </div>
            </div>
          </div>

          <div className="bg-white p-6 rounded-lg shadow-sm">
            <div className="flex items-center">
              <div className="p-2 bg-green-100 rounded-lg">
                <Activity className="h-6 w-6 text-green-600" />
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-gray-600">AI Success Rate</p>
                <p className="text-2xl font-semibold text-gray-900">94%</p>
              </div>
            </div>
          </div>

          <div className="bg-white p-6 rounded-lg shadow-sm">
            <div className="flex items-center">
              <div className="p-2 bg-purple-100 rounded-lg">
                <Settings className="h-6 w-6 text-purple-600" />
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-gray-600">Operations Today</p>
                <p className="text-2xl font-semibold text-gray-900">127</p>
              </div>
            </div>
          </div>

          <div className="bg-white p-6 rounded-lg shadow-sm">
            <div className="flex items-center">
              <div className="p-2 bg-orange-100 rounded-lg">
                <Search className="h-6 w-6 text-orange-600" />
              </div>
              <div className="ml-4">
                <p className="text-sm font-medium text-gray-600">Avg Response</p>
                <p className="text-2xl font-semibold text-gray-900">150ms</p>
              </div>
            </div>
          </div>
        </div>

        {/* Entities Table */}
        <div className="bg-white shadow-sm rounded-lg overflow-hidden">
          <div className="px-6 py-4 border-b border-gray-200">
            <h2 className="text-lg font-medium text-gray-900">Entities</h2>
          </div>

          {isLoading ? (
            <div className="px-6 py-8 text-center">
              <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
              <p className="mt-2 text-gray-600">Loading entities...</p>
            </div>
          ) : error ? (
            <div className="px-6 py-8 text-center text-red-600">
              Error loading entities
            </div>
          ) : entities.length === 0 ? (
            <div className="px-6 py-8 text-center text-gray-500">
              No entities found
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="min-w-full divide-y divide-gray-200">
                <thead className="bg-gray-50">
                  <tr>
                    <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Name
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Type
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Details
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Created
                    </th>
                    <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Actions
                    </th>
                  </tr>
                </thead>
                <tbody className="bg-white divide-y divide-gray-200">
                  {entities.map((entity: Entity) => (
                    <tr key={entity.id} className="hover:bg-gray-50">
                      <td className="px-6 py-4 whitespace-nowrap">
                        <div className="text-sm font-medium text-gray-900">{entity.name}</div>
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap">
                        <span className="inline-flex px-2 py-1 text-xs font-semibold rounded-full bg-blue-100 text-blue-800">
                          {entity.entity_type}
                        </span>
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                        {Object.entries(entity.data).slice(0, 2).map(([key, value]) => (
                          <div key={key}>
                            <span className="font-medium">{key}:</span> {String(value)}
                          </div>
                        ))}
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                        {new Date(entity.created_at).toLocaleDateString()}
                      </td>
                      <td className="px-6 py-4 whitespace-nowrap text-sm font-medium">
                        <button className="text-blue-600 hover:text-blue-900 mr-4">
                          Edit
                        </button>
                        <button className="text-red-600 hover:text-red-900">
                          Delete
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Create Entity Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 bg-gray-600 bg-opacity-50 overflow-y-auto h-full w-full z-50">
          <div className="relative top-20 mx-auto p-5 border w-11/12 md:w-3/4 lg:w-1/2 shadow-lg rounded-md bg-white">
            <div className="mt-3">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-medium text-gray-900">Create Entity with AI</h3>
                <button
                  onClick={() => setShowCreateModal(false)}
                  className="text-gray-400 hover:text-gray-600"
                >
                  ×
                </button>
              </div>

              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">
                    Natural Language Instruction
                  </label>
                  <textarea
                    className="w-full p-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    rows={3}
                    placeholder="e.g., Create a Delaware LLC called TechCorp Solutions for software development"
                    value={naturalLanguageInput}
                    onChange={(e) => setNaturalLanguageInput(e.target.value)}
                  />
                </div>

                <button
                  onClick={handleGenerateDsl}
                  disabled={generateDslMutation.isPending || !naturalLanguageInput.trim()}
                  className="w-full px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {generateDslMutation.isPending ? 'Generating...' : 'Generate DSL'}
                </button>

                {generatedDsl && (
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Generated DSL (Review & Edit if needed)
                    </label>
                    <textarea
                      className="w-full p-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent font-mono text-sm"
                      rows={4}
                      value={generatedDsl}
                      onChange={(e) => setGeneratedDsl(e.target.value)}
                    />
                  </div>
                )}

                <div className="flex space-x-4">
                  <button
                    onClick={() => setShowCreateModal(false)}
                    className="flex-1 px-4 py-2 border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleCreateEntity}
                    disabled={createEntityMutation.isPending || !generatedDsl}
                    className="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {createEntityMutation.isPending ? 'Creating...' : 'Create Entity'}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
