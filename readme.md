# Portable Exporter for Suwayomi

Browse content available on a [Suwayomi server](https://github.com/Suwayomi/Suwayomi-Server) instance and export to CBZ and EPUB files.

### Prerequisites
- A running suwayomi server with titles in your library
- rust

## Running
1. clone this repo
2. create a file in the top level called `.env` that looks something like this
```
DATABASE_URL=sqlite:data/database.db
SUWAYOMI_URL=http://10.10.11.250:4567
LANG=en
SQLX_OFFLINE=true
```
3. run `cargo run`

### Developing

When `SQLX_OFFLINE` is true, sqlx uses the data files in .sqlx to generate types for queries at compile time. If you're going to be changing queries or doing migrations, make sure to set that to false. Migrations are handled with sqlx-cli. 