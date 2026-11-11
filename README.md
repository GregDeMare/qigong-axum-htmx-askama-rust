# Rust + htmx + Askama Starter

HTML-first, designer-friendly, SPA-like UX.

## Run

```bash
cargo run
# open http://127.0.0.1:3000
 # filenames: Is April11Short.mp4 the same as April08Short.mp4 ? det verkar så!
 # Symlink: static/video now points to ~/Library/CloudStorage/Dropbox/Quigong/HoldenQuigong, and static/video/April01.mp4 is reachable through it.
 # localfile: /static/video/April11Short.mp4
 # dropbox: 
 # https://www.dropbox.com/scl/fi/1nkv3brh8ruj3c87clfwt/April04Short.mp4?rlkey=97car8u968b1w0xkbyg7uirxi&dl=0
 # https://www.dropbox.com/scl/fi/89wbsznf58yluvcfjqags/April11Short.mp4?rlkey=tq6o2rzl3wnqlvso6jutevmlx&dl=0
 # https://www.dropbox.com/scl/fi/ph4q5v4109mfbusuvu40a/April08Short.mp4?rlkey=y557s7mzejrbt3mqto3trygsv&dl=0
 # https://www.dropbox.com/scl/fi/qh2ygprv6dyvr8mbczoz2/April06Short.mp4?rlkey=cjv0he2vaw4e2kxjuoje7wehx&dl=0
```

## Local videos

Locally, the videos play from files on this Mac instead of Dropbox.

- `static/video` is a symlink (gitignored) to the offline Dropbox folder:
  ```bash
  ln -s ~/Library/CloudStorage/Dropbox/Quigong/HoldenQuigong static/video
  ```
  Keep that folder set to "Make available offline" in Finder.
- In the local database (`postgresql://localhost/qigong-htmx-askama`), every row's `long_url`/`short_url` was switched to local paths:
  ```sql
  UPDATE models SET
    long_url  = '/static/video/' || name       || '.mp4',
    short_url = '/static/video/' || name_short || '.mp4';
  ```
- New models: enter `/static/video/<file>.mp4` (or a Dropbox link) in the video fields.
- Local paths only work with `shuttle run`. The Shuttle database still uses Dropbox links.

### Backup table `models_backup`

Before that update, the original rows (with the Dropbox links) were copied into a table `models_backup` in the local database:

```sql
CREATE TABLE models_backup AS SELECT * FROM models;
```

The app never reads it. To restore the Dropbox links:

```sql
UPDATE models m SET long_url = b.long_url, short_url = b.short_url
FROM models_backup b WHERE m.id = b.id;
```

To remove it once it's no longer needed: `DROP TABLE models_backup;`
