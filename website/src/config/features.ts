export const FEATURES = [
  {
    title: 'Tiny footprint',
    summary:
      'Installer ≈10 MB and ~80 MB on disk. Built with Rust + Tauri for fast startup and ultra‑low idle memory.',
    tag: 'Lightweight'
  },
  {
    title: 'Truly modern',
    summary:
      'No cockpit‑style complexity. We cut the 99% you never use and focus on everyday workflows for smoother, more intuitive operations.',
    tag: 'Modern'
  },
  {
    title: 'Cross‑platform',
    summary: 'Runs on macOS, Windows, and Linux — use the same tool everywhere.',
    tag: 'Cross‑platform'
  },
  {
    title: 'Database compatibility',
    summary:
      'Supports MySQL, PostgreSQL, ClickHouse, TiDB, SQL Server, and SQLite (actively expanding).',
    tag: 'DBs'
  },
  {
    title: 'Looks great',
    summary:
      'Rich theme system with dark/light and high/low saturation styles for all‑day comfort.',
    tag: 'Themes'
  },
  {
    title: 'AI assistance (experimental)',
    summary:
      'Summarize SQL, explain schemas, and analyze slow queries. Security is under active refinement; local and optional cloud modes planned.',
    tag: 'AI'
  },
  {
    title: 'Completely free',
    summary:
      'No login, no payments, no memberships, no ads. Download and go.',
    tag: 'Free'
  }
] as const;
