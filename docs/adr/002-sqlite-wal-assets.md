# ADR-002: SQLite with WAL Mode and Content-Addressed Assets

## Status

Accepted (2026-08-02)

## Context

PasteCanvas stores structured data (items, boards, nodes) and binary assets (images, PDFs, fonts). Requirements:
- Durable writes: "saved" means committed to disk
- Deduplication: same image saved twice stores one blob
- Concurrent reads during background jobs
- Future sync readiness (revision fields, device_id)
- No server dependency

## Decision

- **SQLite** in WAL mode with foreign keys and busy timeout (5s)
- **Content-addressed asset store**: files at `assets/<sha256[0:2]>/<sha256[2:4]>/<sha256>.<ext>`
- **Atomic write protocol**: temp → flush → hash → rename → DB transaction → commit → success
- **FTS5** with dual indexes: `unicode61` (word-based) + `trigram` (substring/CJK)
- **Soft delete** with `deleted_at` timestamp; physical deletion only on trash empty
- **Migration runner**: sequential, immutable, checksum-verified

## Consequences

- Single-file database is easy to backup (VACUUM INTO)
- WAL allows concurrent readers during job processing
- Content-addressed storage gives free deduplication
- FTS5 trigram enables CJK search without external tokenizer
- Revision fields prepare for future CRDT/sync without current complexity
- No network dependency; fully offline-capable

## Alternatives Considered

- **PostgreSQL**: Requires server process, overkill for single-user
- **Realm/ObjectBox**: Less portable, no SQL query flexibility
- **File-based JSON**: No transactional guarantees, poor query performance at scale
