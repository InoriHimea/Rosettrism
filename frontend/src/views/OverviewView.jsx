import { Activity, BarChart3, Database, Gauge, RefreshCw, Server, Sparkles } from 'lucide-react';
import { formatTimestamp } from '../utils/format.js';

export function OverviewView({ t, stats, cache, setActiveView }) {
  const total = stats?.cache?.upstream_entries ?? cache.length;
  const fresh = stats?.cache?.fresh_upstream_entries ?? cache.filter((entry) => entry.fresh).length;
  const expired = Math.max((stats?.cache?.expired_upstream_entries ?? total - fresh) || 0, 0);
  const freshPercent = total > 0 ? Math.round((fresh / total) * 100) : 0;
  const sourceRows = groupedRows(cache, 'source').slice(0, 7);
  const operationRows = operationSummary(cache, t);
  const recentRows = cache.slice(0, 8);
  const providerHealth = (stats?.provider_health || []).slice(0, 8);
  const fetchStatusRows = (stats?.fetch_run_status_counts || []).map((row) => ({ label: row.status, value: row.count }));

  return (
    <section className="dashboard-grid">
      <article className="chart-panel health-panel" data-testid="cache-freshness-ring">
        <div className="chart-title">
          <div>
            <h2>{t.cacheFreshnessRing}</h2>
            <p>{t.totalEntries}: {total}</p>
          </div>
          <Gauge size={20} />
        </div>
        <div className="donut-row">
          <div
            className="donut"
            style={{ '--fresh': `${freshPercent * 3.6}deg` }}
            aria-label={`${t.freshRatio} ${freshPercent}%`}
          >
            <strong>{freshPercent}%</strong>
            <span>{t.freshRatio}</span>
          </div>
          <div className="legend">
            <LegendDot label={t.fresh} value={fresh} color="green" />
            <LegendDot label={t.expired} value={expired} color="amber" />
            <button className="button-secondary text-action" type="button" onClick={() => setActiveView('cache')}>
              {t.cache}
            </button>
          </div>
        </div>
      </article>

      <article className="chart-panel">
        <div className="chart-title">
          <div>
            <h2>{t.sourceMix}</h2>
            <p>{t.upstreamCache}</p>
          </div>
          <BarChart3 size={20} />
        </div>
        <BarList rows={sourceRows} empty={t.noData} />
      </article>

      <article className="chart-panel">
        <div className="chart-title">
          <div>
            <h2>{t.operationMix}</h2>
            <p>{t.searchOps} / {t.fetchOps}</p>
          </div>
          <Server size={20} />
        </div>
        <BarList rows={operationRows} empty={t.noData} />
      </article>

      <article className="chart-panel wide-panel">
        <div className="chart-title">
          <div>
            <h2>{t.recentActivity}</h2>
            <p>{recentRows.length} {t.entries}</p>
          </div>
          <RefreshCw size={20} />
        </div>
        <div className="spark-grid">
          {recentRows.length === 0 ? (
            <span className="empty-state">{t.noData}</span>
          ) : (
            recentRows.map((entry) => (
              <div className="spark-column" key={entry.id}>
                <span
                  className={entry.fresh ? 'spark-bar fresh-bar' : 'spark-bar expired-bar'}
                  style={{ height: `${Math.max(18, Math.min(92, Math.round((entry.body_len || 1) / 80)))}px` }}
                />
                <small>{entry.source}</small>
              </div>
            ))
          )}
        </div>
      </article>

      <ProviderHealthCards t={t} rows={providerHealth} />

      <AiScoreSparkline t={t} scores={stats?.ai_scores || []} />

      <RecentRunsPanel t={t} runs={stats?.fetch_runs || []} />

      <article className="chart-panel">
        <div className="chart-title">
          <div>
            <h2>{t.errorDistribution}</h2>
            <p>{stats?.cache?.fetch_run_entries ?? 0} {t.entries}</p>
          </div>
          <Server size={20} />
        </div>
        <StatusDistributionChart rows={fetchStatusRows} empty={t.noData} />
      </article>
    </section>
  );
}




export function ProviderHealthCards({ t, rows }) {
  return (
    <article className="chart-panel wide-panel" data-testid="provider-health-cards">
      <div className="chart-title">
        <div>
          <h2>{t.providerCards}</h2>
          <p>{t.providerHealthHint}</p>
        </div>
        <Activity size={20} />
      </div>
      <div className="provider-card-grid">
        {rows.length === 0 ? <span className="empty-state">{t.noData}</span> : null}
        {rows.map((row) => {
          const successPercent = Math.round((row.success_rate || 0) * 100);
          const warningPercent = Math.round((row.warning_rate || 0) * 100);
          const errorPercent = Math.round((row.error_rate || 0) * 100);
          return (
            <div className={`provider-card provider-card-${row.status || 'unknown'}`} key={row.source}>
              <span className={`health-light health-${row.status || 'unknown'}`}><i />{row.status || 'unknown'}</span>
              <strong title={row.source}>{row.source}</strong>
              <div className="provider-card-meter" aria-label={`${t.successRate} ${successPercent}%`}>
                <span style={{ width: `${Math.max(4, successPercent)}%` }} />
              </div>
              <small>{successPercent}% · {Math.round(row.average_duration_ms || 0)} ms · W/E {warningPercent}%/{errorPercent}%</small>
            </div>
          );
        })}
      </div>
    </article>
  );
}

export function AiScoreSparkline({ t, scores }) {
  const points = scores.slice(0, 12).reverse().map((score, index) => ({
    label: score.score_json?.model || `#${index + 1}`,
    value: scoreValue(score.score_json),
  }));
  const width = 260;
  const height = 96;
  const path = sparklinePath(points.map((point) => point.value), width, height);
  return (
    <article className="chart-panel" data-testid="ai-score-sparkline">
      <div className="chart-title">
        <div>
          <h2>{t.aiScoreSparkline}</h2>
          <p>{points.length} {t.entries}</p>
        </div>
        <Sparkles size={20} />
      </div>
      {points.length === 0 ? <span className="empty-state">{t.noData}</span> : (
        <div className="ai-sparkline-card">
          <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={t.aiScoreSparkline}>
            <path className="sparkline-area" d={`${path} L ${width} ${height} L 0 ${height} Z`} />
            <path className="sparkline-line" d={path} />
            {points.map((point, index) => {
              const x = points.length === 1 ? width / 2 : (index / (points.length - 1)) * width;
              const y = height - (Math.max(0, Math.min(100, point.value)) / 100) * (height - 18) - 9;
              return <circle cx={x} cy={y} r="3.5" key={`${point.label}-${index}`} />;
            })}
          </svg>
          <strong>{Math.round(points.at(-1)?.value || 0)}</strong>
        </div>
      )}
    </article>
  );
}

function StatusDistributionChart({ rows, empty }) {
  const total = rows.reduce((sum, row) => sum + row.value, 0);
  if (rows.length === 0 || total === 0) {
    return <span className="empty-state">{empty}</span>;
  }
  let cursor = 0;
  return (
    <div className="status-distribution" data-testid="fetch-status-distribution">
      <div className="status-stack" aria-label="Fetch status distribution">
        {rows.map((row, index) => {
          const width = (row.value / total) * 100;
          const style = { left: `${cursor}%`, width: `${width}%` };
          cursor += width;
          return <span className={`status-slice status-slice-${index % 5}`} style={style} title={`${row.label}: ${row.value}`} key={row.label} />;
        })}
      </div>
      <BarList rows={rows} empty={empty} />
    </div>
  );
}

function sparklinePath(values, width, height) {
  if (values.length === 0) {
    return '';
  }
  return values.map((value, index) => {
    const x = values.length === 1 ? width / 2 : (index / (values.length - 1)) * width;
    const y = height - (Math.max(0, Math.min(100, value)) / 100) * (height - 18) - 9;
    return `${index === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`;
  }).join(' ');
}

function scoreValue(score) {
  const selected = score?.scores?.find((item) => item.index === score.best_index) || score?.scores?.[0];
  const value = selected?.ai_score ?? selected?.heuristic_score ?? score?.score;
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return 0;
  }
  return number <= 10 ? number * 10 : number;
}

export function ProviderHealthPanel({ t, rows }) {
  return (
    <article className="chart-panel wide-panel">
      <div className="chart-title">
        <div>
          <h2>{t.providerHealth}</h2>
          <p>{t.providerHealthHint}</p>
        </div>
        <Activity size={20} />
      </div>
      <div className="provider-health-list">
        <div className="provider-health-row provider-health-head">
          <strong>{t.source}</strong>
          <span>{t.healthStatus}</span>
          <span>{t.successRate}</span>
          <span>{t.avgDuration}</span>
          <span>{t.warningErrorRate}</span>
          <span>{t.lastError}</span>
        </div>
        {rows.length === 0 ? <span className="empty-state">{t.noData}</span> : null}
        {rows.map((row) => {
          const successPercent = Math.round((row.success_rate || 0) * 100);
          const warningPercent = Math.round((row.warning_rate || 0) * 100);
          const errorPercent = Math.round((row.error_rate || 0) * 100);
          return (
            <div className="provider-health-row" key={row.source}>
              <strong title={row.source}>{row.source}</strong>
              <span className={`health-light health-${row.status || 'unknown'}`}>
                <i />{row.status || 'unknown'}
              </span>
              <span>
                <span className="mini-bar" aria-label={`${t.successRate} ${successPercent}%`}>
                  <i style={{ width: `${Math.max(4, successPercent)}%` }} />
                </span>
                {successPercent}% / {row.sample_size}
              </span>
              <span>{row.average_duration_ms == null ? '-' : `${Math.round(row.average_duration_ms)} ms`}</span>
              <span>{warningPercent}% / {errorPercent}%</span>
              <span title={row.last_error || ''}>{row.last_error || '-'}</span>
            </div>
          );
        })}
      </div>
    </article>
  );
}

export function RecentRunsPanel({ t, runs }) {
  return (
    <article className="chart-panel wide-panel">
      <div className="chart-title">
        <div>
          <h2>{t.recentRuns}</h2>
          <p>{runs.length} {t.entries}</p>
        </div>
        <RefreshCw size={20} />
      </div>
      <div className="run-list">
        <div className="run-row run-row-head">
          <strong>{t.queryLabel}</strong>
          <span>{t.source}</span>
          <span>{t.mode}</span>
          <span>{t.status}</span>
          <span>{t.message}</span>
          <span>{t.createdAt}</span>
        </div>
        {runs.length === 0 ? <span className="empty-state">{t.noData}</span> : null}
        {runs.slice(0, 8).map((run) => (
          <div className="run-row" key={run.id}>
            <strong title={run.query}>{run.query || '-'}</strong>
            <span>{run.source || 'aggregate'}</span>
            <span>{run.mode}</span>
            <span className={`status-badge ${run.status === 'success' || run.status === 'cache_store' || run.status === 'cache_hit' ? 'status-fresh' : 'status-expired'}`}>
              {run.status}
            </span>
            <span title={run.message || ''}>{run.message || '-'}</span>
            <span>{formatTimestamp(run.created_at)}</span>
          </div>
        ))}
      </div>
    </article>
  );
}

function LegendDot({ label, value, color }) {
  return (
    <div className="legend-row">
      <span className={`legend-dot ${color}`} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function BarList({ rows, empty }) {
  const max = Math.max(...rows.map((row) => row.value), 1);
  if (rows.length === 0) {
    return <span className="empty-state">{empty}</span>;
  }
  return (
    <div className="bar-list">
      {rows.map((row) => (
        <div className="bar-row" key={row.label}>
          <div className="bar-label">
            <span>{row.label}</span>
            <strong>{row.value}</strong>
          </div>
          <div className="bar-track">
            <span style={{ width: `${Math.max(6, Math.round((row.value / max) * 100))}%` }} />
          </div>
        </div>
      ))}
    </div>
  );
}

function groupedRows(items, key) {
  const counts = new Map();
  for (const item of items) {
    const label = item[key] || 'unknown';
    counts.set(label, (counts.get(label) || 0) + 1);
  }
  return [...counts.entries()]
    .map(([label, value]) => ({ label, value }))
    .sort((left, right) => right.value - left.value);
}

function operationSummary(cache, t) {
  const rows = groupedRows(cache, 'operation');
  const labelMap = {
    search: t.searchOps,
    fetch: t.fetchOps,
  };
  return rows.map((row) => ({
    ...row,
    label: labelMap[row.label] || row.label || t.otherOps,
  }));
}
