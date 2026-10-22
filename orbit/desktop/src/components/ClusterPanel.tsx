import React, { useCallback, useEffect, useRef, useState } from 'react';
import styled from 'styled-components';
import { TauriService, handleTauriError } from '@/services/tauri';
import { ClusterNode, ClusterStatus, Endpoint, ProcessState } from '@/types';

/** How often to re-poll while the panel is visible. */
const REFRESH_INTERVAL_MS = 4000;

const Container = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  font-size: 13px;
`;

const Toolbar = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid #3c3c3c;
  background: #252525;
  flex-wrap: wrap;
`;

const Button = styled.button<{ variant?: 'primary' | 'danger' }>`
  padding: 5px 10px;
  border: none;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  color: #ffffff;
  background: ${props =>
    props.variant === 'primary' ? '#0078d4' : props.variant === 'danger' ? '#a4262c' : '#3c3c3c'};
  transition: filter 0.15s;

  &:hover:not(:disabled) {
    filter: brightness(1.2);
  }

  &:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
`;

const SizeInput = styled.input`
  width: 46px;
  padding: 4px 6px;
  background: #3c3c3c;
  border: 1px solid #5a5a5a;
  border-radius: 4px;
  color: #ffffff;
  font-size: 12px;
`;

const Scroll = styled.div`
  flex: 1;
  overflow: auto;
  padding: 12px;
`;

const Summary = styled.div`
  color: #cccccc;
  margin-bottom: 10px;
  line-height: 1.6;
`;

const RootPath = styled.code`
  color: #9cdcfe;
  word-break: break-all;
`;

const NodeCard = styled.div`
  border: 1px solid #3c3c3c;
  border-radius: 6px;
  padding: 10px 12px;
  margin-bottom: 10px;
  background: #252525;
`;

const NodeHeader = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
`;

const NodeName = styled.span`
  font-weight: 600;
`;

const Badge = styled.span<{ tone: 'good' | 'bad' | 'warn' }>`
  padding: 2px 7px;
  border-radius: 10px;
  font-size: 11px;
  color: #ffffff;
  background: ${props =>
    props.tone === 'good' ? '#107c10' : props.tone === 'warn' ? '#8a6d00' : '#a4262c'};
`;

const PortGrid = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
`;

const Port = styled.span<{ reachable: boolean }>`
  padding: 2px 7px;
  border-radius: 4px;
  font-size: 11px;
  border: 1px solid ${props => (props.reachable ? '#107c10' : '#5a5a5a')};
  color: ${props => (props.reachable ? '#8fd18f' : '#999999')};
`;

const Muted = styled.div`
  color: #999999;
  font-size: 12px;
  margin-top: 6px;
`;

const Banner = styled.div<{ tone: 'error' | 'info' }>`
  margin: 12px;
  padding: 10px 12px;
  border-radius: 4px;
  font-size: 12px;
  line-height: 1.5;
  color: ${props => (props.tone === 'error' ? '#f2a3a5' : '#cccccc')};
  background: ${props =>
    props.tone === 'error' ? 'rgba(209, 52, 56, 0.12)' : 'rgba(255, 255, 255, 0.04)'};
  border: 1px solid
    ${props => (props.tone === 'error' ? 'rgba(209, 52, 56, 0.35)' : '#3c3c3c')};
`;

const LogView = styled.pre`
  margin: 0;
  padding: 10px;
  background: #1a1a1a;
  border: 1px solid #3c3c3c;
  border-radius: 4px;
  max-height: 240px;
  overflow: auto;
  font-size: 11px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
  color: #cccccc;
`;

/** Render a duration in whole units, largest first. */
const formatUptime = (seconds: number): string => {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
};

const isRunning = (process: ProcessState): process is Extract<ProcessState, { state: 'running' }> =>
  process.state === 'running';

/**
 * Alive and answering, alive but answering nothing, or gone. Kept as three
 * states because a process that is up with dead listeners is exactly the case
 * worth spotting, and collapsing it into "running" would hide it.
 */
const nodeHealth = (node: ClusterNode): { tone: 'good' | 'warn' | 'bad'; label: string } => {
  if (!isRunning(node.process)) {
    return { tone: 'bad', label: `exited (was pid ${node.process.pid})` };
  }
  const reachable = node.endpoints.filter((e: Endpoint) => e.reachable).length;
  if (node.endpoints.length === 0) {
    return { tone: 'warn', label: 'running, no port flags found' };
  }
  if (reachable === 0) {
    return { tone: 'warn', label: 'running, no ports answering' };
  }
  return { tone: 'good', label: `serving ${reachable}/${node.endpoints.length} ports` };
};

export const ClusterPanel: React.FC = () => {
  const [status, setStatus] = useState<ClusterStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [size, setSize] = useState(3);
  const [log, setLog] = useState<string | null>(null);
  const [logTitle, setLogTitle] = useState<string>('');
  const [rootInput, setRootInput] = useState('');

  // Avoids a state update after unmount when a poll is still in flight.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const next = await TauriService.getClusterStatus();
      if (!mounted.current) return;
      setStatus(next);
      setError(null);
    } catch (err) {
      if (!mounted.current) return;
      setError(handleTauriError(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  const run = async (action: () => Promise<void>): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      await action();
      await refresh();
    } catch (err) {
      if (mounted.current) setError(handleTauriError(err));
    } finally {
      if (mounted.current) setBusy(false);
    }
  };

  const showLog = (nodeId?: string) =>
    run(async () => {
      const content = await TauriService.getClusterLog(nodeId, 200);
      if (!mounted.current) return;
      setLog(content);
      setLogTitle(nodeId ? `${nodeId}.log` : 'cluster-control.log');
    });

  const applyRoot = () =>
    run(async () => {
      const next = await TauriService.setClusterRoot(rootInput.trim());
      if (!mounted.current) return;
      setStatus(next);
      await TauriService.saveSettings({ cluster_root: next.root });
    });

  // No checkout located: the only useful action is to name one.
  if (error && !status) {
    return (
      <Container>
        <Banner tone="error">{error}</Banner>
        <Toolbar>
          <SizeInput
            as="input"
            style={{ width: '100%', minWidth: 180 }}
            placeholder="/path/to/orbit-rs"
            value={rootInput}
            onChange={e => setRootInput(e.target.value)}
          />
          <Button variant="primary" disabled={busy || !rootInput.trim()} onClick={applyRoot}>
            Use this checkout
          </Button>
        </Toolbar>
      </Container>
    );
  }

  return (
    <Container>
      <Toolbar>
        <label htmlFor="cluster-size" style={{ color: '#cccccc' }}>
          Nodes
        </label>
        <SizeInput
          id="cluster-size"
          type="number"
          min={1}
          max={9}
          value={size}
          onChange={e => setSize(Math.max(1, Math.min(9, Number(e.target.value) || 1)))}
        />
        <Button
          variant="primary"
          disabled={busy}
          onClick={() => run(() => TauriService.startCluster(size))}
          title="Runs scripts/start-cluster.sh; it builds orbit-server in release mode first"
        >
          ▶ Start
        </Button>
        <Button
          variant="danger"
          disabled={busy || !status?.running_nodes}
          onClick={() => run(() => TauriService.stopCluster())}
        >
          ■ Stop
        </Button>
        <Button disabled={busy} onClick={() => void refresh()}>
          ⟳ Refresh
        </Button>
        <Button disabled={busy} onClick={() => showLog()}>
          Control log
        </Button>
      </Toolbar>

      {error && <Banner tone="error">{error}</Banner>}

      <Scroll>
        {status && (
          <Summary>
            <div>
              Checkout: <RootPath>{status.root}</RootPath>
            </div>
            {status.initialized ? (
              <div>
                {status.running_nodes} of {status.nodes.length} node
                {status.nodes.length === 1 ? '' : 's'} running · {status.serving_nodes} serving ·
                checked {new Date(status.checked_at).toLocaleTimeString()}
              </div>
            ) : (
              <div>No cluster has been started in this checkout yet.</div>
            )}
          </Summary>
        )}

        {status?.nodes.map(node => {
          const health = nodeHealth(node);
          return (
            <NodeCard key={node.node_id}>
              <NodeHeader>
                <NodeName>{node.node_id}</NodeName>
                <Badge tone={health.tone}>{health.label}</Badge>
              </NodeHeader>

              {isRunning(node.process) ? (
                <>
                  <Muted style={{ marginTop: 0 }}>
                    pid {node.process.pid} · up {formatUptime(node.process.uptime_seconds)}
                  </Muted>
                  {node.endpoints.length > 0 ? (
                    <PortGrid style={{ marginTop: 8 }}>
                      {node.endpoints.map(endpoint => (
                        <Port
                          key={`${endpoint.protocol}-${endpoint.port}`}
                          reachable={endpoint.reachable}
                          title={
                            endpoint.reachable
                              ? 'Accepted a TCP connection'
                              : 'Did not accept a connection'
                          }
                        >
                          {endpoint.protocol} {endpoint.port}
                        </Port>
                      ))}
                    </PortGrid>
                  ) : (
                    <Muted>
                      No <code>--*-port</code> flags on this process's command line, so its ports
                      are unknown.
                    </Muted>
                  )}
                </>
              ) : (
                <Muted style={{ marginTop: 0 }}>
                  A pid file remains but the process is gone. Ports are not shown: the ones it
                  would have used are a guess, not an observation.
                </Muted>
              )}

              <Button
                style={{ marginTop: 10 }}
                disabled={busy}
                onClick={() => showLog(node.node_id)}
              >
                View log
              </Button>
            </NodeCard>
          );
        })}

        {log !== null && (
          <div style={{ marginTop: 12 }}>
            <NodeHeader>
              <NodeName>{logTitle}</NodeName>
              <Button onClick={() => setLog(null)}>Close</Button>
            </NodeHeader>
            <LogView>{log || '(empty)'}</LogView>
          </div>
        )}
      </Scroll>
    </Container>
  );
};

export default ClusterPanel;
