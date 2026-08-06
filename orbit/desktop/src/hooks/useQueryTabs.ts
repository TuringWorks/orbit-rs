import { useState } from 'react';
import { QueryTab, QueryType } from '@/types';

const DEFAULT_ORBITQL_QUERY = `-- Welcome to Orbit Desktop!
-- Try some OrbitQL with ML functions:

SELECT ML_XGBOOST(
    ARRAY[age, income, credit_score],
    loan_approved
) as model_accuracy
FROM loan_applications;`;

interface UseQueryTabsReturn {
  queryTabs: QueryTab[];
  activeTabIndex: number;
  setActiveTabIndex: (index: number) => void;
  createNewTab: (queryType?: QueryType) => void;
  /** Open a new tab pre-filled with `query` and focus it. */
  openTab: (query: string, queryType: QueryType, name?: string) => void;
  closeTab: (index: number) => void;
  updateTabQuery: (index: number, query: string) => void;
  updateTabState: (index: number, updates: Partial<QueryTab>) => void;
  getCurrentTab: () => QueryTab | undefined;
}

export const useQueryTabs = (): UseQueryTabsReturn => {
  const [queryTabs, setQueryTabs] = useState<QueryTab[]>([
    {
      id: '1',
      name: 'Query 1',
      query: DEFAULT_ORBITQL_QUERY,
      query_type: QueryType.OrbitQL,
      unsaved_changes: false,
      is_executing: false,
    }
  ]);
  const [activeTabIndex, setActiveTabIndex] = useState(0);

  /** Append a tab and focus it, deriving both from the list's current length. */
  const appendTab = (query: string, queryType: QueryType, name?: string) => {
    setQueryTabs(prev => {
      setActiveTabIndex(prev.length);
      return [
        ...prev,
        {
          id: `${Date.now()}-${prev.length}`,
          name: name ?? `Query ${prev.length + 1}`,
          query,
          query_type: queryType,
          unsaved_changes: false,
          is_executing: false,
        },
      ];
    });
  };

  const createNewTab = (queryType: QueryType = QueryType.OrbitQL) => {
    appendTab(queryType === QueryType.Redis ? 'PING' : 'SELECT 1;', queryType);
  };

  const openTab = (query: string, queryType: QueryType, name?: string) => {
    appendTab(query, queryType, name);
  };

  const closeTab = (index: number) => {
    if (queryTabs.length <= 1) return;

    const newTabs = queryTabs.filter((_, i) => i !== index);
    setQueryTabs(newTabs);
    
    if (activeTabIndex >= newTabs.length) {
      setActiveTabIndex(newTabs.length - 1);
    } else if (activeTabIndex > index) {
      setActiveTabIndex(activeTabIndex - 1);
    }
  };

  const updateTabQuery = (index: number, query: string) => {
    setQueryTabs(prev => {
      const newTabs = [...prev];
      newTabs[index] = {
        ...newTabs[index],
        query,
        unsaved_changes: true,
      };
      return newTabs;
    });
  };

  const updateTabState = (index: number, updates: Partial<QueryTab>) => {
    setQueryTabs(prev => {
      const newTabs = [...prev];
      newTabs[index] = {
        ...newTabs[index],
        ...updates,
      };
      return newTabs;
    });
  };

  const getCurrentTab = () => queryTabs[activeTabIndex];

  return {
    queryTabs,
    activeTabIndex,
    setActiveTabIndex,
    createNewTab,
    openTab,
    closeTab,
    updateTabQuery,
    updateTabState,
    getCurrentTab,
  };
};