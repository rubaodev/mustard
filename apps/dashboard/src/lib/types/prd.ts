// PRD Lapidator wire types.
//
// These mirror the Rust `PrdData` struct in
// `src-tauri/src/prd_lapidator.rs`. The Rust side uses
// `#[serde(rename_all = "camelCase")]` end-to-end, so what arrives over
// `invoke('lapidate_prd', ...)` is already camelCase JSON.
//
// Single source of truth: keep field names + optionality identical to the
// Rust struct. If the Rust struct changes, change this file too.

export interface PrdLayers {
  backend: boolean;
  frontend: boolean;
  database: boolean;
  design: boolean;
  docs: boolean;
  testes: boolean;
}

export interface PrdAc {
  title: string;
  command: string;
}

export interface PrdConfront {
  entitiesFound: string[];
  entitiesMissing: string[];
  pathsExist: string[];
  pathsMissing: string[];
}

export interface LapidatedPrd {
  type: 'feature' | 'bugfix';
  slug: string;
  title: string;
  scope: 'light' | 'full';
  summary: string;
  why?: string;
  layers: PrdLayers;
  boundaries: string[];
  checklist: string[];
  acceptanceCriteria: PrdAc[];
  decisionsNotObvious?: string[];
  nonGoals?: string[];
  _confront: PrdConfront;
}
