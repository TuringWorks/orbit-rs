import React from 'react';
import styled from 'styled-components';
import { QueryResult } from '@/types';

const ExportButton = styled.button`
  padding: 6px 12px;
  background: #0078d4;
  color: white;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  margin-left: 12px;
  transition: all 0.2s;

  &:hover {
    background: #106ebe;
  }
`;

const ButtonGroup = styled.div`
  display: flex;
  align-items: center;
  margin-bottom: 12px;
`;

interface QueryResultsTableProps {
  result: QueryResult;
}

const Container = styled.div`
  padding: 16px;
`;

const MetaInfo = styled.div`
  margin-bottom: 12px;
  color: #cccccc;
  font-size: 13px;
`;

const TableContainer = styled.div`
  overflow: auto;
  max-height: 400px;
`;

const Table = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
`;

const TableHeader = styled.th`
  padding: 8px 12px;
  text-align: left;
  border-bottom: 1px solid #3c3c3c;
  font-weight: 600;
  background: #2d2d2d;
`;

const ColumnType = styled.div`
  font-size: 10px;
  color: #888888;
  font-weight: normal;
`;

const TableCell = styled.td`
  padding: 8px 12px;
  border-bottom: 1px solid #3c3c3c;
`;

const TableRow = styled.tr<{ isEven: boolean }>`
  background: ${props => props.isEven ? '#1e1e1e' : '#252525'};
`;

const ErrorMessage = styled.div`
  color: #d13438;
`;

const SuccessMessage = styled.div`
  color: #cccccc;
`;

const createUniqueRowKey = (row: any, rowIndex: number): string => {
  const keyValue = row.id || row.name || Object.values(row)[0] || rowIndex;
  return `${keyValue}-${rowIndex}`;
};

const exportToCSV = (data: QueryResult['data']) => {
  if (!data || !data.columns.length || !data.rows.length) return;

  const headers = data.columns.map(col => col.name).join(',');
  const rows = data.rows.map(row =>
    data.columns.map(col => {
      const value = row[col.name];
      // Escape commas and quotes in CSV
      if (value === null || value === undefined) return '';
      const str = String(value);
      if (str.includes(',') || str.includes('"') || str.includes('\n')) {
        return `"${str.replace(/"/g, '""')}"`;
      }
      return str;
    }).join(',')
  ).join('\n');

  const csv = `${headers}\n${rows}`;
  const blob = new Blob([csv], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `query-results-${new Date().toISOString().slice(0, 10)}.csv`;
  a.click();
  URL.revokeObjectURL(url);
};

const exportToJSON = (data: QueryResult['data']) => {
  if (!data) return;

  const json = JSON.stringify(data, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `query-results-${new Date().toISOString().slice(0, 10)}.json`;
  a.click();
  URL.revokeObjectURL(url);
};

const QueryResultsTable: React.FC<QueryResultsTableProps> = ({ result }) => {
  if (!result.success) {
    return (
      <Container>
        <ErrorMessage>
          <strong>Error:</strong> {result.error || 'Unknown error occurred'}
        </ErrorMessage>
      </Container>
    );
  }

  if (!result.data) {
    return (
      <Container>
        <SuccessMessage>Query executed successfully</SuccessMessage>
      </Container>
    );
  }

  const hasData = result.data.rows.length > 0;

  return (
    <Container>
      <ButtonGroup>
        <MetaInfo>
          Execution time: {result.execution_time?.toFixed(2) || 0}ms
          {result.rows_affected !== undefined && (
            <> • Rows: {result.rows_affected}</>
          )}
          {hasData && (
            <> • Showing {result.data.rows.length} row{result.data.rows.length !== 1 ? 's' : ''}</>
          )}
        </MetaInfo>
        {hasData && (
          <>
            <ExportButton onClick={() => exportToCSV(result.data)}>
              📥 Export CSV
            </ExportButton>
            <ExportButton onClick={() => exportToJSON(result.data)}>
              📥 Export JSON
            </ExportButton>
          </>
        )}
      </ButtonGroup>
      
      {hasData ? (
        <TableContainer>
          <Table>
            <thead>
              <tr>
                {result.data.columns.map(col => (
                  <TableHeader key={col.name}>
                    {col.name}
                    <ColumnType>{col.type}</ColumnType>
                  </TableHeader>
                ))}
              </tr>
            </thead>
            <tbody>
              {result.data.rows.map((row, rowIndex) => (
                <TableRow key={createUniqueRowKey(row, rowIndex)} isEven={rowIndex % 2 === 0}>
                  {result.data.columns.map(col => {
                    const value = row[col.name];
                    const displayValue = value === null || value === undefined 
                      ? <span style={{ color: '#666666', fontStyle: 'italic' }}>NULL</span>
                      : typeof value === 'object'
                      ? JSON.stringify(value)
                      : String(value);
                    
                    return (
                      <TableCell key={col.name} title={typeof value === 'object' ? JSON.stringify(value, null, 2) : String(value)}>
                        {displayValue}
                      </TableCell>
                    );
                  })}
                </TableRow>
              ))}
            </tbody>
          </Table>
        </TableContainer>
      ) : (
        <SuccessMessage style={{ padding: '40px', textAlign: 'center' }}>
          Query executed successfully but returned no rows
        </SuccessMessage>
      )}
    </Container>
  );
};

export default QueryResultsTable;