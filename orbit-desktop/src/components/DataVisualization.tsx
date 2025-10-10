import React, { useEffect, useState } from 'react';
import styled from 'styled-components';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title as ChartTitle,
  Tooltip,
  Legend,
  ArcElement,
  ScatterController,
  Filler,
} from 'chart.js';
import { Line, Bar, Pie, Scatter, Doughnut } from 'react-chartjs-2';
import { QueryResultData, ChartConfig, VisualizationData } from '@/types';

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  ScatterController,
  ChartTitle,
  Tooltip,
  Legend,
  Filler
);

interface DataVisualizationProps {
  data: QueryResultData;
  className?: string;
}

const Container = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #1e1e1e;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  overflow: hidden;
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid #3c3c3c;
  background: #2d2d2d;
`;

const Title = styled.h3`
  margin: 0;
  color: #ffffff;
  font-size: 16px;
  font-weight: 600;
`;

const Controls = styled.div`
  display: flex;
  gap: 8px;
  align-items: center;
`;

const Select = styled.select`
  padding: 4px 8px;
  background: #3c3c3c;
  border: 1px solid #5a5a5a;
  border-radius: 4px;
  color: #ffffff;
  font-size: 12px;
  outline: none;

  &:focus {
    border-color: #0078d4;
  }

  option {
    background: #3c3c3c;
    color: #ffffff;
  }
`;

const Button = styled.button<{ active?: boolean }>`
  padding: 4px 8px;
  background: ${props => props.active ? '#0078d4' : '#3c3c3c'};
  color: white;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.2s;

  &:hover {
    background: ${props => props.active ? '#106ebe' : '#484848'};
  }
`;

const ChartContainer = styled.div`
  flex: 1;
  padding: 16px;
  position: relative;
  min-height: 300px;
  
  canvas {
    max-height: 100% !important;
  }
`;

const ConfigPanel = styled.div<{ show: boolean }>`
  position: absolute;
  top: 0;
  right: 0;
  width: 250px;
  height: 100%;
  background: #2d2d2d;
  border-left: 1px solid #3c3c3c;
  padding: 16px;
  transform: ${props => props.show ? 'translateX(0)' : 'translateX(100%)'};
  transition: transform 0.3s ease;
  z-index: 10;
  overflow-y: auto;

  h4 {
    margin: 0 0 12px 0;
    color: #ffffff;
    font-size: 14px;
    font-weight: 600;
  }

  .config-group {
    margin-bottom: 16px;
    
    label {
      display: block;
      margin-bottom: 4px;
      color: #cccccc;
      font-size: 12px;
    }
  }
`;

const EmptyState = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #888888;
  
  .icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
  }
  
  .message {
    font-size: 16px;
    margin-bottom: 8px;
  }
  
  .submessage {
    font-size: 14px;
    opacity: 0.7;
    text-align: center;
  }
`;

const chartColors = [
  '#0078d4', '#107c10', '#d83b01', '#5c2d91', '#e81123',
  '#00bcf2', '#bad80a', '#ff8c00', '#c239b3', '#00b7c3'
];

export const DataVisualization: React.FC<DataVisualizationProps> = ({
  data,
  className
}) => {
  const [chartType, setChartType] = useState<'line' | 'bar' | 'pie' | 'scatter' | 'doughnut'>('bar');
  const [config, setConfig] = useState<ChartConfig>({
    type: 'bar',
    title: 'Data Visualization',
    x_axis: data.columns[0]?.name || '',
    y_axis: data.columns[1]?.name || '',
    color_scheme: chartColors,
    show_legend: true,
    show_grid: true,
  });
  const [showConfig, setShowConfig] = useState(false);

  // Auto-detect chart configuration based on data
  useEffect(() => {
    if (data.columns.length >= 2) {
      const numericColumns = data.columns.filter(col => 
        col.type.includes('int') || col.type.includes('float') || col.type.includes('decimal') || col.type.includes('numeric')
      );
      
      const dateColumns = data.columns.filter(col =>
        col.type.includes('date') || col.type.includes('time')
      );

      const stringColumns = data.columns.filter(col =>
        col.type.includes('text') || col.type.includes('varchar') || col.type.includes('char')
      );

      // Smart defaults
      let newConfig = { ...config };
      
      if (dateColumns.length > 0 && numericColumns.length > 0) {
        // Time series data - use line chart
        setChartType('line');
        newConfig.type = 'line';
        newConfig.x_axis = dateColumns[0].name;
        newConfig.y_axis = numericColumns[0].name;
      } else if (stringColumns.length > 0 && numericColumns.length > 0) {
        // Categorical data - use bar chart
        setChartType('bar');
        newConfig.type = 'bar';
        newConfig.x_axis = stringColumns[0].name;
        newConfig.y_axis = numericColumns[0].name;
      } else if (numericColumns.length >= 2) {
        // Two numeric columns - use scatter plot
        setChartType('scatter');
        newConfig.type = 'scatter';
        newConfig.x_axis = numericColumns[0].name;
        newConfig.y_axis = numericColumns[1].name;
      }
      
      setConfig(newConfig);
    }
  }, [data]);

  const isNumericColumn = (columnName: string) => {
    const column = data.columns.find(col => col.name === columnName);
    return column && (
      column.type.includes('int') || 
      column.type.includes('float') || 
      column.type.includes('decimal') || 
      column.type.includes('numeric')
    );
  };

  const isDateColumn = (columnName: string) => {
    const column = data.columns.find(col => col.name === columnName);
    return column && (column.type.includes('date') || column.type.includes('time'));
  };

  const processChartData = () => {
    if (!data.rows.length) return null;

    const xData = data.rows.map(row => row[config.x_axis]);
    const yData = data.rows.map(row => row[config.y_axis]);

    switch (chartType) {
      case 'line':
      case 'bar':
        return {
          labels: xData,
          datasets: [{
            label: config.y_axis,
            data: yData,
            backgroundColor: chartType === 'bar' ? config.color_scheme[0] + '80' : 'transparent',
            borderColor: config.color_scheme[0],
            borderWidth: 2,
            fill: chartType === 'line' ? false : true,
            tension: chartType === 'line' ? 0.4 : 0,
            pointBackgroundColor: config.color_scheme[0],
            pointBorderColor: config.color_scheme[0],
            pointRadius: chartType === 'line' ? 4 : 0,
          }]
        };

      case 'scatter':
        return {
          datasets: [{
            label: `${config.x_axis} vs ${config.y_axis}`,
            data: data.rows.map(row => ({
              x: row[config.x_axis],
              y: row[config.y_axis]
            })),
            backgroundColor: config.color_scheme[0] + '80',
            borderColor: config.color_scheme[0],
            borderWidth: 2,
            pointRadius: 5,
          }]
        };

      case 'pie':
      case 'doughnut':
        // Group by x_axis and sum y_axis
        const grouped = data.rows.reduce((acc, row) => {
          const key = row[config.x_axis];
          if (!acc[key]) acc[key] = 0;
          acc[key] += parseFloat(row[config.y_axis]) || 0;
          return acc;
        }, {} as Record<string, number>);

        const labels = Object.keys(grouped);
        const values = Object.values(grouped);

        return {
          labels,
          datasets: [{
            data: values,
            backgroundColor: config.color_scheme.slice(0, labels.length),
            borderColor: '#1e1e1e',
            borderWidth: 2,
          }]
        };

      default:
        return null;
    }
  };

  const getChartOptions = () => {
    const baseOptions = {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        title: {
          display: !!config.title,
          text: config.title,
          color: '#ffffff',
          font: {
            size: 16,
            weight: 'bold' as const,
          }
        },
        legend: {
          display: config.show_legend,
          labels: {
            color: '#ffffff',
          }
        },
        tooltip: {
          backgroundColor: '#2d2d2d',
          titleColor: '#ffffff',
          bodyColor: '#cccccc',
          borderColor: '#3c3c3c',
          borderWidth: 1,
        }
      },
      scales: chartType !== 'pie' && chartType !== 'doughnut' ? {
        x: {
          title: {
            display: true,
            text: config.x_axis,
            color: '#ffffff',
          },
          ticks: {
            color: '#cccccc',
            maxRotation: 45,
          },
          grid: {
            display: config.show_grid,
            color: '#3c3c3c',
          }
        },
        y: {
          title: {
            display: true,
            text: config.y_axis,
            color: '#ffffff',
          },
          ticks: {
            color: '#cccccc',
          },
          grid: {
            display: config.show_grid,
            color: '#3c3c3c',
          }
        }
      } : undefined
    };

    return baseOptions;
  };

  const chartData = processChartData();

  const renderChart = () => {
    if (!chartData) return null;

    const options = getChartOptions();

    switch (chartType) {
      case 'line':
        return <Line data={chartData} options={options} />;
      case 'bar':
        return <Bar data={chartData} options={options} />;
      case 'pie':
        return <Pie data={chartData} options={options} />;
      case 'scatter':
        return <Scatter data={chartData} options={options} />;
      case 'doughnut':
        return <Doughnut data={chartData} options={options} />;
      default:
        return null;
    }
  };

  if (data.rows.length === 0) {
    return (
      <Container className={className}>
        <EmptyState>
          <div className="icon">📊</div>
          <div className="message">No Data to Visualize</div>
          <div className="submessage">Execute a query that returns data to create visualizations</div>
        </EmptyState>
      </Container>
    );
  }

  const numericColumns = data.columns.filter(col => isNumericColumn(col.name));
  const categoricalColumns = data.columns.filter(col => !isNumericColumn(col.name));

  return (
    <Container className={className}>
      <Header>
        <Title>Data Visualization</Title>
        <Controls>
          <Button 
            active={chartType === 'line'} 
            onClick={() => setChartType('line')}
            disabled={numericColumns.length === 0}
          >
            📈 Line
          </Button>
          <Button 
            active={chartType === 'bar'} 
            onClick={() => setChartType('bar')}
          >
            📊 Bar
          </Button>
          <Button 
            active={chartType === 'pie'} 
            onClick={() => setChartType('pie')}
          >
            🥧 Pie
          </Button>
          <Button 
            active={chartType === 'scatter'} 
            onClick={() => setChartType('scatter')}
            disabled={numericColumns.length < 2}
          >
            ⚫ Scatter
          </Button>
          <Button 
            active={chartType === 'doughnut'} 
            onClick={() => setChartType('doughnut')}
          >
            🍩 Doughnut
          </Button>
          <Button onClick={() => setShowConfig(!showConfig)}>
            ⚙️ Config
          </Button>
        </Controls>
      </Header>

      <ChartContainer>
        {renderChart()}
        
        <ConfigPanel show={showConfig}>
          <h4>Chart Configuration</h4>
          
          <div className="config-group">
            <label>Title</label>
            <input
              type="text"
              value={config.title}
              onChange={(e) => setConfig({ ...config, title: e.target.value })}
              style={{
                width: '100%',
                padding: '6px 8px',
                background: '#3c3c3c',
                border: '1px solid #5a5a5a',
                borderRadius: '4px',
                color: '#ffffff',
                fontSize: '12px'
              }}
            />
          </div>

          <div className="config-group">
            <label>X Axis</label>
            <Select
              value={config.x_axis}
              onChange={(e) => setConfig({ ...config, x_axis: e.target.value })}
            >
              {data.columns.map(col => (
                <option key={col.name} value={col.name}>
                  {col.name} ({col.type})
                </option>
              ))}
            </Select>
          </div>

          <div className="config-group">
            <label>Y Axis</label>
            <Select
              value={config.y_axis}
              onChange={(e) => setConfig({ ...config, y_axis: e.target.value })}
            >
              {data.columns.map(col => (
                <option key={col.name} value={col.name}>
                  {col.name} ({col.type})
                </option>
              ))}
            </Select>
          </div>

          <div className="config-group">
            <label>
              <input
                type="checkbox"
                checked={config.show_legend}
                onChange={(e) => setConfig({ ...config, show_legend: e.target.checked })}
                style={{ marginRight: '8px' }}
              />
              Show Legend
            </label>
          </div>

          <div className="config-group">
            <label>
              <input
                type="checkbox"
                checked={config.show_grid}
                onChange={(e) => setConfig({ ...config, show_grid: e.target.checked })}
                style={{ marginRight: '8px' }}
              />
              Show Grid
            </label>
          </div>

          <div className="config-group">
            <label>Data Summary</label>
            <div style={{ fontSize: '11px', color: '#888888' }}>
              Rows: {data.rows.length}<br />
              Columns: {data.columns.length}<br />
              Numeric: {numericColumns.length}<br />
              Categorical: {categoricalColumns.length}
            </div>
          </div>
        </ConfigPanel>
      </ChartContainer>
    </Container>
  );
};