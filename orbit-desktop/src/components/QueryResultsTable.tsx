import React from 'react';
import styled from 'styled-components';
import { QueryResult } from '@/types';

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

const QueryResultsTable: React.FC<QueryResultsTableProps> = ({ result }) => {
  if (!result.success) {
    return (
      <Container>
        <ErrorMessage>Error: {result.error}</ErrorMessage>
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

  return (
    <Container>
      <MetaInfo>
        Execution time: {result.execution_time?.toFixed(2) || 0}ms
        {result.rows_affected && (
          <> • Rows affected: {result.rows_affected}</>
        )}
      </MetaInfo>
      
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
                {result.data.columns.map(col => (
                  <TableCell key={col.name}>
                    {String(row[col.name] ?? 'NULL')}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </tbody>
        </Table>
      </TableContainer>
    </Container>
  );
};

export default QueryResultsTable;