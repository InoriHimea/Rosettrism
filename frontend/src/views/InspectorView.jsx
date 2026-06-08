import { formatTimestamp } from '../utils/format.js';

export function InspectorView({ t, result, stats }) {
  const parsed = parseJsonSafe(result);
  const directScore = parsed?.ai_score || parsed?.aiScore;
  const recentScore = stats?.ai_scores?.[0]?.score_json;
  const score = directScore || recentScore || null;
  const history = buildAiScoreHistory(stats?.ai_scores || [], score);
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
              <span><strong>{t.promptVersion}</strong>{score.prompt_version || '-'}</span>
              <span><strong>{t.configHash}</strong>{score.config_hash || '-'}</span>
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
        {history.length > 0 && (
          <div className="quality-card">
            <h3>{t.aiScoreHistory}</h3>
            <div className="quality-table">
              <div className="quality-row quality-head">
                <span>{t.createdAt}</span>
                <span>{t.aiModelUsed}</span>
                <span>{t.promptVersion}</span>
                <span>{t.bestCandidate}</span>
              </div>
              {history.map((item) => {
                const best = item.scores?.find((candidate) => candidate.index === item.best_index);
                const changed = score && (item.best_index !== score.best_index || item.model !== score.model || item.prompt_version !== score.prompt_version);
                return (
                  <div className={`quality-row ${changed ? 'quality-selected' : ''}`} key={`${item.created_at}-${item.model}-${item.prompt_version}-${item.best_index}`}>
                    <span>{formatTimestamp(item.created_at)}</span>
                    <span>
                      <strong>{item.model || '-'}</strong>
                      <small>{item.base_url || '-'}</small>
                    </span>
                    <span>{item.prompt_version || '-'}</span>
                    <span>
                      <strong>{best?.source || `#${item.best_index}`}</strong>
                      <small>{formatScore(best?.ai_score)} · {best?.title || '-'}</small>
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
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


function buildAiScoreHistory(records, currentScore) {
  const scores = records
    .map((record) => record?.score_json || record)
    .filter(Boolean);
  if (currentScore && !scores.some((score) => sameScore(score, currentScore))) {
    scores.unshift(currentScore);
  }
  return scores.slice(0, 10);
}

function sameScore(left, right) {
  return left?.candidate_summary_hash === right?.candidate_summary_hash
    && left?.model === right?.model
    && left?.prompt_version === right?.prompt_version
    && left?.created_at === right?.created_at;
}
