# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A small server-rendered CRUD app for cataloguing Qigong class videos: Rust + Axum 0.8 + Askama templates + htmx 2 (loaded from unpkg in `templates/base.html`), backed by Postgres via sqlx. It used to be deployed on Shuttle, which has shut down. It is now a plain Tokio/Axum binary, and no new host has been chosen yet.

## Commands

```bash
cargo run            # needs DATABASE_URL (from the environment or .env; see .env.example)
cargo build          # compile check (Askama templates are checked at compile time)
cargo clippy
```

There are no tests in the repo.

Configuration is read from environment variables, with `.env` loaded via `dotenvy`. `DATABASE_URL` is required (locally `postgresql://localhost/qigong-htmx-askama`). `HOST` (default `127.0.0.1`), `PORT` (default `8000`) and `RUST_LOG` (default `info`) are optional. `.env` is tracked in git, so put shared defaults in `.env.example` and keep secrets out of `.env` commits.

`backup/models.sql` is a data-only `pg_dump` of the `models` table, taken 2026-11-12. Restore it into a database that has the schema with `psql "$DATABASE_URL" -f backup/models.sql`, after emptying `models`, because the dump inserts explicit ids.

## Architecture

Everything lives in `src/main.rs`: data structs, one Askama template struct per template file, the router (`build_router`), and handlers.

- **Schema/seed: sqlx migrations.** `main` runs `sqlx::migrate!()`, which embeds `migrations/*.sql` at compile time, so the deployed binary doesn't need the `migrations/` folder. (The earlier runtime `Migrator::new(Path)` attempt failed on Shuttle for that reason.) `build.rs` rebuilds when `migrations/` changes. Applied migrations are recorded in `_sqlx_migrations` with a checksum. **Never edit or delete an applied migration**, because startup will fail. Add a new one instead: `sqlx migrate add -s <name>` (needs `sqlx-cli`). `0001`/`0002` are idempotent (`IF NOT EXISTS`, `ON CONFLICT DO NOTHING`) because the databases existed before migrations were introduced. Keep machine-specific data, such as local `/static/video` paths, out of migrations.
- **Single table `models`:** the column list is repeated literally in every SELECT/INSERT/UPDATE `RETURNING` clause and mirrored in `Model` (`FromRow`), `ModelForm` (`Deserialize`), the seed SQL, and the form templates. Adding a column means updating all of them.
- **htmx fragment pattern:** handlers return either full pages (templates that `{% extends "base.html" %}`) or HTML fragments (templates prefixed `_`):
  - `GET /models` returns the full `models.html` page, unless the request has `HX-Request` **and** `HX-Target: rows`. In that case it returns only `_rows.html` (the `<tbody id="rows">` contents). The filter form in `models.html` relies on this.
  - Create: `/models/new` renders `_form.html` into `#modal`. On POST it returns a single `_row.html` `<tr>` that is appended `beforeend` to `#rows`.
  - Edit: `/models/{id}/edit` swaps the row (`#row-{id}`) for the inline `_form_row.html`. PUT returns the updated `_row.html`.
  - Delete returns an empty body so the `outerHTML` swap removes the row.
  - Video: `/models/{id}/video?kind=long|short` returns `_video_row.html`, inserted `afterend` of the clicked row. Inline `hx-on`/`onclick` in `_row.html` removes any open video row first.
- **Videos:** `long_url`/`short_url` hold Dropbox links in every database. `playback_src` takes the file name from the link (`video_file_name`). If `static/video/<file>` exists and `VideoStatus` says it is offline, it plays the local file. Otherwise it streams the Dropbox link via `normalize_dropbox_url`, which rewrites `dl=0`/`dl=1` to `raw=1`. Locally, `static/video` is a gitignored symlink to the user's Dropbox folder (`~/Library/CloudStorage/Dropbox/Quigong/HoldenQuigong`). On a server it doesn't exist, so Dropbox is always used. `VideoStatus` reads the macOS `SF_DATALESS` flag that Dropbox sets on online-only files, and reports offline on other platforms. It is shown on `/videos` and as icons in the models table. Plain `/static/video/x.mp4` values are still accepted. Poster images come from `static/img/{image_name}`. `/static` is served by `tower_http::ServeDir`.
- List filtering and sorting is built with sqlx `QueryBuilder`. Sort keys are whitelisted in a `match`, and user input is bound with `push_bind`.
- `templates/products.html` is a leftover from the starter template; it has no route.

## Known rough edges

- Handlers `.unwrap()` DB and render results, so errors panic the request instead of returning an error response.
- Hardcoded `colspan` values (7/8) don't match the 10-column table.
