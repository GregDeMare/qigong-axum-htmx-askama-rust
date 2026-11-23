# Qigong video catalog

A Rust, Axum, htmx, Askama, and PostgreSQL app for managing long and short Qigong videos. The model list is the home page, with inline editing, modal creation, filtering, and local or remote playback.

## Run

Start PostgreSQL and create the database once if it does not exist:

```bash
createdb qigong-htmx-askama
cp .env.example .env   # first setup only; adjust DATABASE_URL as needed
cargo run
```

Open **http://127.0.0.1:8000**. The database user must be able to create tables and sequences and apply migrations. Startup applies pending migrations before the server starts listening. Restart `cargo run` after updating Rust code, templates, or migrations; templates and migrations are compiled into the binary.

| Variable | Purpose | Default |
| --- | --- | --- |
| `DATABASE_URL` | PostgreSQL connection URL | Required; `.env.example` uses `postgresql://localhost/qigong-htmx-askama` |
| `HOST` | Address to listen on | `127.0.0.1`; use `0.0.0.0` when required by a host |
| `PORT` | HTTP port | `8000` |
| `RUST_LOG` | Logging filter | `info` |

The app loads `.env` automatically; existing environment variables take precedence. `.env` is gitignored. Command-line tools such as `psql` do not load `.env` automatically.

## Managing videos

- **New Model** opens a modal. A successful create closes it and refreshes the list.
- **Edit** opens an inline form. **Save** updates the model; **Cancel** reloads only that model's saved row and discards unsaved edits. Modal Cancel closes the modal without a database request.
- **Delete** asks for confirmation. The row disappears after a successful database delete.
- Click a long or short video name to open its player below the row. New htmx requests close existing players; **Escape** also closes video rows.
- The header's **🌓** button switches between dark and light themes. The choice is saved in this browser's local storage; dark is the default.
- CSS and JavaScript under `/static` use `Cache-Control: no-cache`, so browsers revalidate those files when requested.

### Filters, sorting, and navigation

Search matches the long name, short name, or description without regard to case. **Min score** filters the long video's score; clearing it removes the threshold. Sort by name, long-video score, or long-video play count in either direction. The default order is model ID ascending.

Filter changes update the list URL, so a filtered page can be refreshed or bookmarked. For example:

```text
http://127.0.0.1:8000/?q=April&min_score=4&sort=name_desc
```

Opening forms, saving, cancelling, deleting, and opening video players keep the current page URL. Successful creates, updates, and deletes refresh the table with the current filters and sort order. A saved model may therefore move to a different position or disappear if it no longer matches the filters. Failed saves leave the form and its entries in place.

## Form validation

The same server-side rules apply to both create and update. Validation runs before database writes, even when browser validation is bypassed. All detected field-rule errors are returned together.

| Field | Rule | Saving behavior |
| --- | --- | --- |
| Name (`name`) | Required; whitespace alone is invalid | Leading and trailing whitespace is removed |
| Short (`name_short`) | Required; whitespace alone is invalid | Leading and trailing whitespace is removed |
| #Times (`num_of_times`) | Integer from `0` to `2147483647` | Zero is allowed |
| #Times short (`num_of_times_short`) | Integer from `0` to `2147483647` | Zero is allowed |
| Long video (`long_url`) | Required; supported HTTP/HTTPS URL or local MP4 path | Leading and trailing whitespace is removed |
| Short video (`short_url`) | Required; same URL rules as Long video | Leading and trailing whitespace is removed |
| Score / Score short | Signed 32-bit integers | No additional score range is imposed |
| Description / Comments | May be empty | Whitespace and text are preserved |
| Image filename | May be empty | No additional filename validation is applied |

For successfully parsed forms, invalid names, negative play counts, and invalid video URLs return **HTTP 422**. Errors appear beside each affected field, with an error summary. Invalid fields have `aria-invalid="true"`, link to their error text with `aria-describedby`, and are highlighted in both themes. Original submitted values remain available for correction; trimming happens only after these checks pass. Invalid updates leave the saved row unchanged.

Missing form fields, malformed integers, and integers outside the signed 32-bit range are rejected by Axum's form parser before field validation. These parser rejections do not use the field-specific error form. All fields must be included when submitting requests directly, including empty description, comments, and image filename values.

### Supported video URLs

Remote links must have an explicit `http://` or `https://` scheme and a host, and must parse as a URL. Scheme matching is case-insensitive. Embedded whitespace, control characters, backslashes, missing hosts, and malformed authority prefixes such as `https:///example.com/video.mp4` are rejected. A remote URL does not have to end in `.mp4`; Dropbox query strings are supported.

Local paths must begin with `/static/video/` and contain one nonempty filename ending in `.mp4` (case-insensitive). Spaces in local filenames and query strings/fragments are supported. Subdirectories, traversal paths, backslashes, and encoded slash/backslash separators (`%2f`, `%5c`, any case) in the filename are rejected.

Accepted examples:

```text
https://www.dropbox.com/scl/fi/abc/April01.mp4?rlkey=example&dl=0
http://localhost:8000/static/video/April01.mp4
/static/video/April01.mp4
/static/video/April 01.MP4
```

Rejected examples:

```text
javascript:alert(1)
file:///tmp/video.mp4
https://
/static/video/../video.mp4
/static/video/page.html
```

Validation checks the format only. It does not contact remote servers or require a local file to exist. Existing records are not rewritten or validated by a migration; editing them applies the current rules.

## Error handling

| Situation | Response and visible behavior |
| --- | --- |
| Invalid field rules | 422; errors and entered values stay in the create modal or inline edit form |
| Database failure during create/update | 500; a retry message and form values remain available |
| Database failure during delete | 500; an error is shown and the row stays visible |
| Database failure during reads | 500; an error is shown instead of an empty list or a request panic |
| Missing model | 404; a message is shown; an attempted update preserves unsaved entries |
| Template rendering failure | 500; a generic display error is returned |
| Connection failure / timeout | The browser shows a request error; after a timeout, check the list before retrying |

Database and rendering details are logged in the app terminal. User-facing responses contain a general message. Invalid nonblank `min_score` query values are rejected by the query parser; a blank or omitted threshold is allowed.

## Local videos

Keep Dropbox links in the database to use the same records locally and on a server. Playback extracts the MP4 filename from a link. If that file exists in `static/video` and is offline, the app plays the local copy. Otherwise it uses the original URL, converting Dropbox `dl=0` or `dl=1` to `raw=1` for streaming. A server without local videos uses the remote links.

On this Mac, `static/video` can be a gitignored symlink to the Dropbox folder:

```bash
ln -s ~/Library/CloudStorage/Dropbox/Quigong/HoldenQuigong static/video
```

Choose **Make available offline** in Finder for files you want to play locally. A plain `/static/video/filename.mp4` value requires that file on the machine running the app; it has no remote fallback.

The **Local videos** page (`/videos`) lists MP4 filenames, sizes, availability, which models use them, and the offline count. Model rows also show availability icons:

| Icon | Meaning |
| --- | --- |
| 💾 | Local file available offline |
| ☁️ | Dropbox online-only file, detected using macOS file metadata |
| ⚠️ | Local file missing |
| 🌐 | Remote URL without a local copy |

On other operating systems, existing local files are treated as offline files. Poster images are served from `static/img` using each model's image filename.

## Database IDs and migrations

Migration [0003_models_id_sequence.sql](migrations/0003_models_id_sequence.sql) adds the integer sequence `public.models_id_seq`, owned by `public.models.id`, and installs `nextval` as the column default. It locks writes during initialization and positions the sequence so the next generated ID is above existing IDs, or `1` for an empty table or one containing only nonpositive IDs.

Creates omit the ID column, including when a submitted form contains an ID. PostgreSQL allocates IDs safely for concurrent requests. Updates use the ID in the route. Sequence gaps are normal, including after rejected inserts; IDs are not row counts.

Connect from a terminal:

```bash
psql postgresql://localhost/qigong-htmx-askama
```

Inside `psql`, check migration installation and the ID default:

```sql
SELECT current_database();
SELECT version, description, success, installed_on
FROM _sqlx_migrations ORDER BY version;
```

```text
\d public.models
```

Migration version `3` should have `success = true` (`t` in psql). The `id` default should be `nextval('models_id_seq'::regclass)`, possibly schema-qualified. The startup notice that `_sqlx_migrations` already exists is normal. The app only logs `listening` after migrations succeed. Exit psql with `\q`.

## Tests and manual checks

Run the Rust tests with a PostgreSQL user allowed to create databases. SQLx creates isolated test databases:

```bash
DATABASE_URL=postgresql://localhost/postgres cargo test
```

Coverage includes concurrent IDs, empty tables, database failures and retries, missing records, Cancel row fragments, cleared filters, sorted lists after mutations, URL validation, field-error markup, preservation of invalid inputs, unchanged rows after rejected updates, and trimming valid values while preserving notes.

In Chrome, start from the list page and check:

| Action | Expected result |
| --- | --- |
| Submit spaces as Name or Short | A field error; entered values remain |
| Submit an unsupported video URL | An error beside the affected video field |
| Clear Min score and search/sort | A working filtered list with its query in the URL |
| Open a form or video player | The list URL stays unchanged |
| Edit a disposable model and Cancel | Its saved row returns without saving edits |
| Save while a filter/sort is active | The table refreshes in the selected order; nonmatching rows are hidden |
| Cancel a new model | The modal closes without creating a row |
| Refresh a filtered page | The full page, filter values, and matching rows load |
| Switch theme and reload | The selected theme remains active |

The browser normally blocks negative play counts. To test server validation directly, this request intentionally submits a blank Name and a negative count and must return 422 without inserting a row:

```bash
curl -i http://127.0.0.1:8000/models \
  --data-urlencode 'name=   ' \
  --data-urlencode 'name_short=Validation test' \
  --data-urlencode 'description=Keep this description' \
  --data-urlencode 'comments=Keep these comments' \
  --data-urlencode 'num_of_times=-1' \
  --data-urlencode 'num_of_times_short=0' \
  --data-urlencode 'score=0' \
  --data-urlencode 'score_short=0' \
  --data-urlencode 'image_name=' \
  --data-urlencode 'long_url=https://example.com/long.mp4' \
  --data-urlencode 'short_url=/static/video/short.mp4'
```

### Test a database write failure

In the local test database, add a temporary constraint in psql:

```sql
ALTER TABLE public.models
ADD CONSTRAINT test_save_error
CHECK (name <> '__db_error_test__') NOT VALID;
```

In Chrome, create a model with exactly `__db_error_test__` in **Name** (two underscores on each side, no extra characters), a valid Short name, nonnegative counts, and valid video URLs. The database should reject it with 500 and the modal should preserve its entries. An update of a disposable model to that name should behave similarly inline. Use **DevTools → Network** to see the status and the app terminal to see the database error.

`NOT VALID` skips checking existing rows; it still checks new inserts and updates. The constraint checks Name, not Short. Inspect it with:

```sql
SELECT conname, convalidated, pg_get_constraintdef(oid)
FROM pg_constraint
WHERE conrelid = 'public.models'::regclass
  AND conname = 'test_save_error';
```

Remove it after testing:

```sql
ALTER TABLE public.models DROP CONSTRAINT IF EXISTS test_save_error;
```

## Data backup and restore

`backup/models.sql` is a data-only dump of the models table from 2026-11-12. Set the connection URL in your shell; psql and pg_dump do not automatically read `.env`:

```bash
export DATABASE_URL=postgresql://localhost/qigong-htmx-askama
```

To refresh the backup:

```bash
pg_dump --data-only --column-inserts --table=public.models "$DATABASE_URL" > backup/models.sql
```

To restore, first let the app create the schema and apply migration 0003, then stop the app. The following command replaces existing model rows and advances the sequence past the explicitly imported IDs:

```bash
psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 --single-transaction \
  -c "TRUNCATE public.models;" -f backup/models.sql \
  -c "SELECT setval('public.models_id_seq', GREATEST(COALESCE(MAX(id), 1), 1), COALESCE(MAX(id), 0) >= 1) FROM public.models;"
```

The dump changes the session search path, so the sequence reset uses schema-qualified names. Reset the sequence after other imports that supply explicit IDs as well. The dump does not include local video files or poster images.

### Local backup table

The local database also has a historical `models_backup` table copied on 2026-11-11. The app does not read it. To remove it once it is no longer needed:

```sql
DROP TABLE public.models_backup;
```

Developer details, routes, and maintenance conventions are in [CLAUDE.md](CLAUDE.md).
