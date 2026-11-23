# CLAUDE.md

Developer guidance for this repository. [README.md](README.md) contains setup instructions, user features, validation examples, manual checks, and database operations.

## Project and commands

A server-rendered Qigong video catalog built with Rust, Axum 0.8, Askama, htmx 2, and PostgreSQL through SQLx. It runs as a plain Tokio/Axum binary; the previous Shuttle deployment is no longer used.

```bash
cargo run            # DATABASE_URL comes from the environment or .env
cargo build          # also checks Askama templates at compile time
cargo clippy
DATABASE_URL=postgresql://localhost/postgres cargo test
```

The SQLx tests create isolated databases and need a PostgreSQL user with permission to create databases. Unit tests also cover URL validation, cleared filters, and validation responses before database access. Database tests cover concurrent IDs, empty tables, failed writes and retries, missing models, row restoration, filtered/sorted refreshes, rejected forms leaving rows unchanged, and normalization of valid forms.

`DATABASE_URL` is required. `HOST`, `PORT`, and `RUST_LOG` default to `127.0.0.1`, `8000`, and `info`. Real environment variables take precedence over `.env`, which is optional and gitignored. Keep shared defaults in `.env.example`. The database user must have the permissions needed to apply the schema and sequence migrations.

## Source map

| Location | Responsibility |
| --- | --- |
| `src/main.rs` | Data structs, form rules, template structs, router, handlers, errors, playback helpers, tests |
| `templates/base.html` | Page shell, theme toggle, htmx loading, modal shell, global error area, video lifecycle handlers |
| `templates/models.html` | List controls, filter history, and refreshable `#rows` table body |
| `templates/_model_fields.html` | Shared create/edit inputs, retained values, field errors, and accessibility attributes |
| `templates/_form.html` / `_form_row.html` | Create modal and inline edit wrappers |
| `templates/_row.html` / `_rows.html` | A model row and the list-row fragment |
| `static/js/request-errors.js` | Display marked application error fragments and general request errors |
| `static/js/model-navigation.js` | Refresh the active list after successful mutations |
| `static/css/app.css` | Dark/light color variables, responsive fields, validation highlighting, and video styling |
| `migrations/` / `build.rs` | Embedded schema/data migrations and rebuild-on-migration-change handling |
| `backup/models.sql` | Historical data-only dump with explicit model IDs |

The model column list is repeated in SQL and mirrored in `Model`, `ModelForm`, seed data, and templates. Adding a column requires updating these uses. `templates/products.html` is an unused starter template without a route.

## Routes and response shapes

| Method | Route | Result |
| --- | --- | --- |
| GET | `/`, `/models` | Full model list, or only rows when both `HX-Request` and `HX-Target: rows` are present |
| POST | `/models` | Create; return a row on success or the populated modal on an application error |
| GET | `/models/new` | New-model modal fragment |
| GET | `/models/{id}` | Full detail page |
| PUT | `/models/{id}` | Update; return a row on success or populated inline edit form on an application error |
| DELETE | `/models/{id}` | Empty successful body, allowing htmx to remove the row |
| GET | `/models/{id}/edit` | Inline edit form |
| GET | `/models/{id}/row` | Single persisted row, used by inline Cancel |
| GET | `/models/{id}/video?kind=long\|short` | Video row fragment inserted below the model |
| GET | `/videos` | Local MP4 availability and usage list |
| GET | `/static/...` | Files served by `ServeDir`, with `Cache-Control: no-cache` |

Do not send an entire list page to a single-row target. Fragment endpoints are used by htmx rather than as page-history destinations.

## Navigation, filters, and mutation refreshes

Only the list filter form explicitly sets `hx-push-url="true"`. The page body uses `hx-boost="true"` without a page-wide push override. The table and create actions disable URL pushes for fragments. Normal header navigation still uses boosted page links.

`ListParams` contains `q`, `min_score`, and `sort`. Search uses bound `ILIKE` patterns across long name, short name, and description. A blank or absent minimum score means no threshold; a malformed nonblank value is rejected by Axum's query parser. The threshold applies to the long score. Sort choices are whitelisted name/score/play-count ascending or descending; the default is ID ascending. Filtering uses SQLx `QueryBuilder` and parameter binding.

The create form, update form, and delete button carry `data-model-mutation`. `model-navigation.js` handles `htmx:afterRequest` using the original `requestConfig.elt`, which remains identifiable even if its form or row was removed. Only successful marked requests trigger `modelsChanged` on `#rows`. The table body then GETs `/` with `hx-include="#model-filters"`, targets itself, and replaces its contents. Keep this refresh out of browser history and preserve the marker on retry forms. Failed requests must not trigger the refresh, which would discard unsaved edits.

Create success appends the returned row and closes the modal; the subsequent list refresh reapplies filtering and sorting. Update success replaces the row before the refresh. Delete success removes the row before refresh. A changed model may move or cease to match the current filters. Inline Cancel GETs `/models/{id}/row` to discard edits and restore saved data. Modal Cancel closes locally without a request.

## Form validation and normalization

The request is first deserialized into `ModelForm`. Its numeric fields are signed 32-bit integers, and its string fields must be present, even when empty. Malformed/missing fields or out-of-range integers are rejected by the form extractor before the custom validator; these rejections do not contain field-error markup.

For parsed forms, both create and update call `ModelForm::validate()` before any write:

- `name` and `name_short` must contain non-whitespace text.
- Both play counts must be nonnegative; zero is valid.
- Both video URLs must satisfy `valid_video_url()` after trimming their outer whitespace.
- Scores have no additional range rule. Description, comments, and image filename may be empty; no image filename check is performed.

`FormErrors` collects all six possible field-rule errors in one pass. A nonempty result returns 422 through the populated modal or inline form. Only a valid form reaches `normalize()`, which trims names and video URLs. It preserves description/comment whitespace and does not alter the image filename. Do not normalize invalid submissions before rendering them back to the user.

Remote video URLs require an explicit HTTP/HTTPS scheme (case-insensitive), a parsed host, and no embedded whitespace, control characters, backslashes, or malformed authority prefix. They use the `url` crate rather than only a string prefix check, and may contain Dropbox queries or paths without an MP4 suffix. Local paths require the exact `/static/video/` prefix and one nonempty filename ending in `.mp4`, case-insensitively. Spaces, queries, and fragments are supported; directory separators, encoded slash/backslash separators in the filename, and control characters are rejected. Validation does not contact remote servers or check local file existence.

Both form wrappers include `_model_fields.html`, so keep field markup and rules consistent there. Error inputs use `aria-invalid` and `aria-describedby`; messages have `role="alert"`. Prefixes `create` and `edit-{id}` avoid collisions between modal and inline error IDs. CSS uses theme-specific `--error` colors. Browser `required` and `min="0"` checks supplement the server rules.

Updates force the form ID to the route ID, including on retries. Creates ignore submitted IDs and let PostgreSQL use the default. No migration retroactively validates historical records, and direct SQL imports bypass these application-level field rules.

## Errors and HTTP status

Handlers use `HandlerResult = Result<Html<String>, RequestError>`. `render()` converts template failures into logged 500 responses. Database failures are logged with their operation or model ID and return general user-facing messages. Read failures are errors rather than empty result sets.

| Condition | Status | Swap target |
| --- | --- | --- |
| Field-rule validation error | 422 | `#modal` / `#row-{id}`, `outerHTML` |
| Create/update database error | 500 | Populated form, same targets as validation |
| Missing update target | 404 | Populated inline form, preserving unsaved entries |
| Other missing model | 404 | Global `#request-error`, `innerHTML` |
| Read/delete database failure or rendering failure | 500 | Global `#request-error`, `innerHTML` |

`RequestError::into_response()` sets `X-App-Error: true`, `HX-Retarget`, `HX-Reswap`, and `HX-Push-Url: false`. `request-errors.js` allows swapping only these marked application errors during `htmx:beforeSwap`. It keeps htmx's failed-request status, so mutation refreshes and modal-success handling do not run for errors. Delete failures leave the model row present. Global messages are cleared on new requests; connection failures, timeouts, and unmarked HTTP errors have general messages.

## Schema, IDs, and backups

`main` awaits `sqlx::migrate!().run()` before binding the HTTP listener. Migrations are embedded, so deployed binaries do not need a migrations directory. `build.rs` rebuilds when migration files change. `_sqlx_migrations` records checksums and success; an existing-table startup notice is normal.

Never edit or delete an applied migration. Add a new one, for example with `sqlx migrate add -s <name>` if sqlx-cli is installed. `0001` and `0002` contain idempotent schema/seed operations because databases predated the migration mechanism. Keep machine-specific video paths out of migrations.

`0003_models_id_sequence.sql` creates an integer sequence owned by `models.id`, initializes it from the maximum existing ID under an exclusive table lock in SQLx's migration transaction, and installs the column default. The next value is above existing IDs, or 1 for empty/nonpositive-only tables. Concurrent creates rely on `nextval`, never on an application-side `MAX(id)+1`. Failed inserts may consume values; sequence gaps are expected.

The historical dump supplies explicit IDs. After imports, reset the sequence above the imported maximum. Use the schema-qualified restore command in README: the dump clears `search_path`, so unqualified `models` or `models_id_seq` names can fail afterward. The restore example replaces current rows in a single transaction and uses `ON_ERROR_STOP`. Backups do not contain local videos or images.

## Playback, themes, and static files

`playback_src()` extracts an MP4 filename from the URL and chooses a local offline file if present. Otherwise it uses the source URL, changing Dropbox `dl=0`/`dl=1` to `raw=1`. Retain remote links in records shared with a server. `/static/video/...` values require files on that server and have no remote fallback.

`VideoStatus` uses macOS's `SF_DATALESS` flag to detect Dropbox online-only files. Other platforms treat existing files as offline. The UI shows offline, online-only, missing, and remote icons. `/videos` lists MP4 files, sizes, usage, and an offline count. The local `static/video` symlink is gitignored. Posters use `static/img/{image_name}`. Global page handlers close/pause video rows on requests and close them on Escape.

Dark is the default theme. The header toggle sets `document.documentElement.dataset.theme` and stores `qigong-theme` in local storage; head code restores it before paint. `/static` responses request cache revalidation. Restart the running binary after Rust, template, or migration changes.

## Remaining limitations

- Form-extractor errors do not yet provide field-specific retry forms.
- Scores have no business-rule range, and URLs are checked for syntax rather than availability.
- Some list/video `colspan` values still differ from the ten-column table.
