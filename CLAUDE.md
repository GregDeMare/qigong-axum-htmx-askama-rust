# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A small server-rendered CRUD app for cataloguing Qigong class videos: Rust + Axum 0.8 + Askama templates + htmx 2 (loaded from unpkg in `templates/base.html`), backed by Postgres via sqlx, deployed on Shuttle (project name `models-app` in `Shuttle.toml`).

## Commands

```bash
shuttle run          # local run; Shuttle provisions/attaches Postgres (see local_uri below)
shuttle deploy       # deploy to Shuttle
cargo build          # compile check (Askama templates are checked at compile time)
cargo clippy
```

There are no tests in the repo.

The entry point is `#[shuttle_runtime::main]`, so plain `cargo run` does not start a server. For local runs, the `#[shuttle_shared_db::Postgres(local_uri = "postgresql://localhost/qigong-htmx-askama")]` attribute in `src/main.rs` points at a local Postgres database that must exist.

## Architecture

Everything lives in `src/main.rs`: data structs, one Askama template struct per template file, the router (`build_router`), handlers, and `bootstrap`.

- **Schema/seed:** `bootstrap()` runs on every startup. It does `CREATE TABLE IF NOT EXISTS models` and inserts seed rows with `ON CONFLICT (id) DO NOTHING`. The `migrations/` SQL files duplicate this but are **not run**. sqlx migrations didn't work on Shuttle deploy (see commit 4980974), so schema changes go into `bootstrap()`. Keep `migrations/` in sync only if you want it as documentation.
- **Single table `models`:** the column list is repeated literally in every SELECT/INSERT/UPDATE `RETURNING` clause and mirrored in `Model` (`FromRow`), `ModelForm` (`Deserialize`), the seed SQL, and the form templates. Adding a column means updating all of them.
- **htmx fragment pattern:** handlers return either full pages (templates that `{% extends "base.html" %}`) or HTML fragments (templates prefixed `_`):
  - `GET /models` returns the full `models.html` page, unless the request has `HX-Request` **and** `HX-Target: rows`. In that case it returns only `_rows.html` (the `<tbody id="rows">` contents). The filter form in `models.html` relies on this.
  - Create: `/models/new` renders `_form.html` into `#modal`. On POST it returns a single `_row.html` `<tr>` that is appended `beforeend` to `#rows`.
  - Edit: `/models/{id}/edit` swaps the row (`#row-{id}`) for the inline `_form_row.html`. PUT returns the updated `_row.html`.
  - Delete returns an empty body so the `outerHTML` swap removes the row.
  - Video: `/models/{id}/video?kind=long|short` returns `_video_row.html`, inserted `afterend` of the clicked row. Inline `hx-on`/`onclick` in `_row.html` removes any open video row first.
- **Videos** are hosted on Dropbox (`long_url`/`short_url`). `normalize_dropbox_url` rewrites `dl=0`/`dl=1` to `raw=1` so the `<video>` tag can stream them. Poster images come from `static/img/{image_name}`. `/static` is served by `tower_http::ServeDir`. Locally, `static/video` is a gitignored symlink to the user's offline Dropbox folder (`~/Library/CloudStorage/Dropbox/Quigong/HoldenQuigong`). The URL fields can therefore hold either a Dropbox link or a local path like `/static/video/April01.mp4`. Non-http values skip `normalize_dropbox_url` and are used as-is. Local paths only work under `shuttle run`, not on deploy. The local database rows have been switched to `/static/video/` paths. The original Dropbox links are kept in a `models_backup` table, which the app doesn't use. The README "Local videos" section has the restore SQL. `VideoStatus` checks each local file's macOS `SF_DATALESS` flag to tell whether Dropbox has it offline. On other platforms the check always reports offline. The result is shown on the `/videos` page and as icons next to the names in the models table.
- List filtering and sorting is built with sqlx `QueryBuilder`. Sort keys are whitelisted in a `match`, and user input is bound with `push_bind`.
- `templates/products.html` is a leftover from the starter template; it has no route.

## Known rough edges

- Handlers `.unwrap()` DB and render results, so errors panic the request instead of returning an error response.
- Hardcoded `colspan` values (7/8) don't match the 10-column table.
