# 2026-06-21 Frontend Playback and Source Picker Closure

## Background

- Current `master` is tagged as `v4.8.19`; new uncommitted work focuses on lyric playback rendering, source selection UX, Playwright assertions, screenshots, and rebuilt dashboard assets.
- The older UI/UX modernization plan still mapped `v4.8.19` to design-token work even though `v4.8.19` has already shipped as the quality coverage release.
- README roadmap still lists several items that have since been implemented, including provider health and cache maintenance commands.

## Goals

- Release the current frontend rendering and source-picker improvements as `v4.8.20`.
- Keep karaoke rendering product behavior covered by unit and Playwright tests.
- Update shipped `frontend/dist` assets after source and version changes.
- Reconcile plan and README roadmap drift so future work starts from the current state.

## Non-Goals

- Do not complete the whole UI/UX modernization roadmap in one release.
- Do not redesign Settings or add AI score replay in this release.
- Do not commit live provider capture output under `frontend/verification/`.

## Phase 1 - Plan and Requirement Alignment

### Task Checklist

- [x] Create this plan.
- [x] Append `requirement.md` with the 4.8.20 frontend closure requirement.
- [x] Update the older UI/UX plan to remove the `v4.8.19` numbering conflict.
- [x] Refresh README roadmap items that are already implemented.

### Acceptance

- [x] Plan and requirement records point to the same release scope.
- [x] Future roadmap labels do not conflict with already published `v4.8.19`.

## Phase 2 - Version and Asset Closure

### Task Checklist

- [x] Update version metadata to `4.8.20` in `Cargo.toml`, `Cargo.lock`, `frontend/package.json`, and `frontend/package-lock.json`.
- [x] Run `npm run build` so `frontend/dist` matches the source.
- [x] Keep generated live verification output out of the release.

### Acceptance

- [x] `frontend/dist/index.html` references the current hashed bundle.
- [x] Old hashed dist assets are removed and new hashed dist assets are tracked.

## Phase 3 - Validation

### Task Checklist

- [x] `cargo fmt --check`
- [ ] `cargo test --no-fail-fast`
- [x] `npm run build`
- [x] `npm run test:unit`
- [x] `npm test`
- [x] `npm run verify:meta-stress`
- [x] `git diff --check`

### Acceptance

- [x] Required checks pass, or exact environment blockers are recorded.
- [x] Playwright screenshot artifacts are intentional and aligned with the rendering change.

## Verification Record

| Date | Command / Check | Result | Notes |
|------|-----------------|--------|-------|
| 2026-06-21 | `npm run build` | Passed | Rebuilt dashboard bundle as `index-CBH9gdZg.js` and `index-mhHWS6y-.css`. |
| 2026-06-21 | `cargo fmt --check` | Passed | No formatting diff. |
| 2026-06-21 | `cargo test --no-fail-fast` | Environment blocked | 147 lib tests, 14 CLI tests, and schema tests passed; Windows Application Control Policy blocked `tests/decode_fixture.rs` executable with `os error 4551`. |
| 2026-06-21 | `npm run test:unit` | Passed | 11 Node tests passed. |
| 2026-06-21 | `npm test` | Passed | 7 Playwright tests passed. |
| 2026-06-21 | `npm run verify:meta-stress` | Passed | Meta panel, countdown bubbles, annotations, and ending samples passed. |
| 2026-06-21 | `git diff --check` | Passed | CRLF conversion warnings only; no whitespace errors. |
| 2026-06-21 | In-app Browser check at `http://127.0.0.1:5173` | Passed | Dashboard loaded, Fetch source picker opened with 10 options, menu stayed above actions, and no horizontal overflow was detected. |
| 2026-06-21 | `scripts/check-plan-requirement.sh` via Git Bash | Passed | Feature-sensitive changes are accompanied by plan and requirement updates. |

## Overall Status

- [ ] Not started
- [ ] In progress
- [x] Complete
- [ ] Cancelled
