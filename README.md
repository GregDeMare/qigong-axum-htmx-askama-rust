# Rust + htmx + Askama Starter

HTML-first, designer-friendly, SPA-like UX.

## Run

```bash
cp .env.example .env   # once; sets DATABASE_URL
cargo run
# open http://127.0.0.1:8000
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

The same code and data work locally and on a server. The database keeps the Dropbox links. When a video is played, the server takes the file name from the link (`…/April01.mp4?rlkey=…`). If that file is in `static/video` **and Dropbox has it offline**, it plays the local file. Otherwise it streams from Dropbox. On a server there is no `static/video`, so it always uses Dropbox.

- `static/video` is a symlink (gitignored) to the Dropbox folder:
  ```bash
  ln -s ~/Library/CloudStorage/Dropbox/Quigong/HoldenQuigong static/video
  ```
  Choose "Make available offline" in Finder for the videos you want to play locally. Online-only files stream from Dropbox instead of downloading on demand.
- The **Local videos** page (`/videos`) and the icons in the models table show which files are offline (💾) or online-only (☁️).
- New models: enter the Dropbox link. A plain `/static/video/<file>.mp4` path also works, but only on this Mac.

## Data backup

`backup/models.sql` is a data-only dump of the `models` table (2026-11-12). To restore it into a database whose schema the app has already created:

```bash
psql "$DATABASE_URL" -c "TRUNCATE models;" -f backup/models.sql
```

To refresh it:

```bash
pg_dump --data-only --column-inserts --table=models "$DATABASE_URL" > backup/models.sql
```


### Backup table `models_backup`

The local database has a table `models_backup`, a copy of `models` taken on 2026-11-11. The app never reads it. Its rows hold the Dropbox links from that date, apart from manual edits made earlier. They were used to restore the links when local playback switched to the automatic approach above.

To remove it once it's no longer needed: `DROP TABLE models_backup;`
