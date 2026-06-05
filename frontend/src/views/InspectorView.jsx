import { formatTimestamp } from '../utils/format.js';

export function InspectorView({ t, result, stats }) {
  const parsed = parseJsonSafe(result);
  const directScore = parsed?.ai_score || parsed?.aiScore;
  const recentScore = stats?.ai_scores?.[0]?.score_json;
  const score = directScore || recentScore || null;
  const selected = score?.scores?.find((item) => item.index === score.best_index);

  return (
    <section className="panel split-panel">
      <div>
        <h2>{t.inspector}</h2>
        {score ? (
          <div className="quality-card">
            <h3>{t.aiScoreDetails}</h3>
            <div className="quality-meta">
              <span><strong>{t.finalSource}</strong>{selected?.source || '-'}</span>
              <span><strong>{t.aiModelUsed}</strong>{score.model || '-'} · {score.base_url || '-'}</span>
              <span><strong>{t.candidateHash}</strong>{score.candidate_summary_hash || '-'}</span>
              <span><strong>{t.createdAt}</strong>{formatTimestamp(score.created_at)}</span>
            </div>
            <div className="quality-table">
              <div className="quality-row quality-head">
                <span>{t.candidateSource}</span>
                <span>{t.heuristicScore}</span>
                <span>{t.aiScore}</span>
                <span>{t.aiReason}</span>
              </div>
              {(score.scores || []).map((candidate) => (
                <div className={`quality-row ${candidate.index === score.best_index ? 'quality-selected' : ''}`} key={`${candidate.index}-${candidate.source}`}>
                  <span>
                    <strong>{candidate.source || '-'}</strong>
                    <small>{[candidate.title, candidate.artist].filter(Boolean).join(' · ') || `#${candidate.index}`}</small>
                  </span>
                  <span>{formatScore(candidate.heuristic_score)}</span>
                  <span>{formatScore(candidate.ai_score)}</span>
                  <span>{candidate.reason || '-'}</span>
                </div>
              ))}
            </div>
          </div>
        ) : (
          <p className="hint">{t.qualityPending}</p>
        )}
      </div>
      <pre className="result compact">{result || (recentScore ? JSON.stringify(recentScore, null, 2) : t.resultEmpty)}</pre>
    </section>
  );
}

function parseJsonSafe(value) {
  if (!value) {
    return null;
  }
  try {
    return typeof value === 'string' ? JSON.parse(value) : value;
  } catch {
    return null;
  }
}

function formatScore(value) {
  if (value === null || value === undefined || Number.isNaN(Number(value))) {
    return '-';
  }
  return Number(value).toFixed(1);
}
