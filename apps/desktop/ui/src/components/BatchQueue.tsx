import React from 'react';

export interface BatchJobItem {
  id: string;
  request: {
    input_path: string;
    output_path: string;
    model_id: string;
    film_grain: number;
    temporal_mode?: string;
  };
  state: 'Queued' | 'Processing' | 'Completed' | 'Failed' | 'Cancelled';
  progress?: {
    current_frame: number;
    total_frames: number;
    percentage: number;
    current_fps: number;
    eta_seconds: number;
  };
  error_message?: string;
  created_at_unix: number;
}

interface BatchQueueProps {
  jobs: BatchJobItem[];
  isActive: boolean;
  onStartQueue: () => void;
  onPauseQueue: () => void;
  onClearCompleted: () => void;
  onRemoveJob: (id: string) => void;
  onAddFiles: () => void;
}

export const BatchQueue: React.FC<BatchQueueProps> = ({
  jobs,
  isActive,
  onStartQueue,
  onPauseQueue,
  onClearCompleted,
  onRemoveJob,
  onAddFiles,
}) => {
  const getStatusBadge = (state: BatchJobItem['state']) => {
    switch (state) {
      case 'Queued':
        return <span className="status-badge status-queued">Queued</span>;
      case 'Processing':
        return <span className="status-badge status-processing">Processing</span>;
      case 'Completed':
        return <span className="status-badge status-completed">Completed</span>;
      case 'Failed':
        return <span className="status-badge status-failed">Failed</span>;
      case 'Cancelled':
        return <span className="status-badge status-cancelled">Cancelled</span>;
    }
  };

  const getFileName = (path: string) => {
    const parts = path.split(/[\\/]/);
    return parts[parts.length - 1] || path;
  };

  return (
    <div className="queue-panel">
      {/* Top Action Bar */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '16px',
        }}
      >
        <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
          <button className="btn-secondary" onClick={onAddFiles}>
            📂 Add Videos to Queue...
          </button>
          {isActive ? (
            <button className="btn-secondary" onClick={onPauseQueue}>
              ⏸ Pause Queue
            </button>
          ) : (
            <button
              className="btn-primary"
              style={{ width: 'auto', padding: '8px 16px' }}
              disabled={jobs.length === 0}
              onClick={onStartQueue}
            >
              ▶ Start Batch Queue ({jobs.filter((j) => j.state === 'Queued').length} Pending)
            </button>
          )}
        </div>

        <button
          className="btn-secondary"
          onClick={onClearCompleted}
          disabled={!jobs.some((j) => j.state === 'Completed' || j.state === 'Cancelled')}
        >
          🗑 Clear Completed
        </button>
      </div>

      {/* Table */}
      {jobs.length > 0 ? (
        <div
          style={{
            border: '1px solid var(--border-color)',
            borderRadius: '6px',
            overflow: 'hidden',
          }}
        >
          <table className="queue-table">
            <thead>
              <tr>
                <th>Video File</th>
                <th>Model</th>
                <th>Temporal Mode</th>
                <th>Status</th>
                <th>Progress</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {jobs.map((job) => (
                <tr key={job.id}>
                  <td>
                    <div style={{ fontWeight: 600, color: '#f3f4f6' }}>
                      {getFileName(job.request.input_path)}
                    </div>
                    <div style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
                      → {getFileName(job.request.output_path)}
                    </div>
                  </td>
                  <td>
                    <span className="meta-chip">
                      <strong>{job.request.model_id}</strong>
                    </span>
                  </td>
                  <td>
                    <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
                      {job.request.temporal_mode || 'Disabled'}
                    </span>
                  </td>
                  <td>{getStatusBadge(job.state)}</td>
                  <td style={{ minWidth: '180px' }}>
                    {job.progress ? (
                      <div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', marginBottom: '4px' }}>
                          <span>{job.progress.percentage.toFixed(1)}%</span>
                          <span style={{ color: 'var(--text-muted)' }}>
                            {job.progress.current_frame}/{job.progress.total_frames}
                          </span>
                        </div>
                        <div
                          style={{
                            height: '6px',
                            backgroundColor: 'var(--bg-tertiary)',
                            borderRadius: '3px',
                            overflow: 'hidden',
                          }}
                        >
                          <div
                            style={{
                              width: `${job.progress.percentage}%`,
                              height: '100%',
                              backgroundColor:
                                job.state === 'Completed'
                                  ? 'var(--success-color)'
                                  : job.state === 'Failed'
                                  ? '#ef4444'
                                  : 'var(--accent-color)',
                            }}
                          />
                        </div>
                      </div>
                    ) : (
                      <span style={{ color: 'var(--text-muted)' }}>—</span>
                    )}
                  </td>
                  <td>
                    <button
                      className="btn-danger"
                      style={{ padding: '4px 8px', fontSize: '11px' }}
                      disabled={job.state === 'Processing'}
                      onClick={() => onRemoveJob(job.id)}
                    >
                      Remove
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div
          style={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '12px',
            color: 'var(--text-muted)',
            textAlign: 'center',
            padding: '40px',
          }}
        >
          <div style={{ fontSize: '32px' }}>📋</div>
          <div style={{ fontSize: '15px', fontWeight: 600, color: '#f3f4f6' }}>
            Batch Queue is Empty
          </div>
          <div style={{ fontSize: '13px', maxWidth: '400px' }}>
            Click <strong>"Add Videos to Queue..."</strong> to pick multiple files with the native OS file picker and batch render them sequentially.
          </div>
        </div>
      )}
    </div>
  );
};
