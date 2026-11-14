use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;

use super::{legacy, BASELINE, CORE};

const RELEASES: &[(&str, &str)] = &[
    ("v0.1.30", include_str!("fixtures/v0.1.30.sql")),
    ("v0.1.38", include_str!("fixtures/v0.1.38.sql")),
];

type Column = (String, String, bool, Option<String>, i64);
type Index = (String, bool, Vec<String>, Option<String>);
type ForeignKey = (String, String, Option<String>, String);

#[derive(Debug, PartialEq)]
struct Table {
    columns: BTreeSet<Column>,
    indexes: BTreeSet<Index>,
    foreign_keys: BTreeSet<ForeignKey>,
}

fn strings(conn: &Connection, sql: &str, arg: &str) -> Vec<String> {
    conn.prepare(sql)
        .unwrap()
        .query_map([arg], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn describe(conn: &Connection) -> BTreeMap<String, Table> {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    tables
        .into_iter()
        .map(|name| {
            let table = Table {
                columns: columns(conn, &name),
                indexes: indexes(conn, &name),
                foreign_keys: foreign_keys(conn, &name),
            };
            (name, table)
        })
        .collect()
}

fn columns(conn: &Connection, table: &str) -> BTreeSet<Column> {
    conn.prepare("SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?1)")
        .unwrap()
        .query_map([table], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, String>(1)?.to_ascii_uppercase(),
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn indexes(conn: &Connection, table: &str) -> BTreeSet<Index> {
    let listed: Vec<(String, bool)> = conn
        .prepare("SELECT name, \"unique\" FROM pragma_index_list(?1) WHERE origin = 'c'")
        .unwrap()
        .query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    listed
        .into_iter()
        .map(|(name, unique)| {
            let keys = strings(
                conn,
                "SELECT COALESCE(name, '<expr>') FROM pragma_index_xinfo(?1) WHERE key = 1 ORDER BY seqno",
                &name,
            );
            let sql = conn
                .query_row("SELECT sql FROM sqlite_schema WHERE name = ?1", [&name], |r| r.get::<_, Option<String>>(0))
                .unwrap()
                .map(|sql| sql.split_whitespace().collect::<Vec<_>>().join(" "));
            (name, unique, keys, sql)
        })
        .collect()
}

fn foreign_keys(conn: &Connection, table: &str) -> BTreeSet<ForeignKey> {
    conn.prepare("SELECT \"from\", \"table\", \"to\", on_delete FROM pragma_foreign_key_list(?1)")
        .unwrap()
        .query_map([table], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn fresh() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    kroma_sqlite::migrate(&mut conn, &CORE, None).unwrap();
    conn
}

fn dump(conn: &Connection) -> String {
    conn.prepare(
        "SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY rowid",
    )
    .unwrap()
    .query_map([], |r| r.get::<_, String>(0))
    .unwrap()
    .map(|sql| format!("{};\n", sql.unwrap()))
    .collect()
}

fn mismatches(expected: &BTreeMap<String, Table>, found: &BTreeMap<String, Table>) -> Vec<String> {
    let names: BTreeSet<&String> = expected.keys().chain(found.keys()).collect();
    names
        .into_iter()
        .filter(|name| expected.get(*name) != found.get(*name))
        .map(|name| {
            format!(
                "{name}: expected {:#?}\nfound {:#?}",
                expected.get(name),
                found.get(name)
            )
        })
        .collect()
}

#[test]
fn the_baseline_is_the_schema_the_legacy_migrations_build() {
    let conn = Connection::open_in_memory().unwrap();
    legacy::adopt(&conn).unwrap();

    let built = dump(&conn);

    if std::env::var_os("KROMA_BLESS_SCHEMA").is_some() {
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/src/schema/baseline.sql"),
            &built,
        )
        .unwrap();
        return;
    }
    assert!(
        built == BASELINE,
        "schema/baseline.sql is stale; regenerate it with \
         KROMA_BLESS_SCHEMA=1 cargo test -p kroma-db the_baseline_is"
    );
}

#[test]
fn a_database_adopted_from_the_legacy_schema_matches_a_fresh_one() {
    let mut adopted = Connection::open_in_memory().unwrap();
    legacy::adopt(&adopted).unwrap();
    adopted
        .pragma_update(None, "user_version", CORE.baseline_version)
        .unwrap();

    kroma_sqlite::migrate(&mut adopted, &CORE, None).unwrap();

    assert_eq!(
        mismatches(&describe(&fresh()), &describe(&adopted)),
        Vec::<String>::new()
    );
}

#[test]
fn every_release_since_v0_1_30_upgrades_to_the_schema_a_fresh_install_gets() {
    let expected = describe(&fresh());

    for (release, schema) in RELEASES {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema).unwrap();

        kroma_sqlite::migrate(&mut conn, &CORE, None).unwrap();

        assert_eq!(
            mismatches(&expected, &describe(&conn)),
            Vec::<String>::new(),
            "{release}"
        );
        assert_eq!(
            kroma_sqlite::user_version(&conn).unwrap(),
            CORE.latest(),
            "{release}"
        );
    }
}

#[test]
fn every_step_has_a_unique_name_that_starts_with_its_version() {
    let names: Vec<&str> = CORE.steps.iter().map(|step| step.name).collect();

    for (at, name) in names.iter().enumerate() {
        let version = CORE.baseline_version as usize + at + 1;
        assert!(
            name.starts_with(&format!("{version:04}_")),
            "{name} should start with {version:04}_"
        );
    }
    assert_eq!(names.iter().collect::<BTreeSet<_>>().len(), names.len());
}

#[test]
fn the_hdr_and_audio_step_marks_the_files_it_can_say_more_about_without_unprobing_them() {
    const STEP: &str = "0015_hdr_audio_facts";
    let at = CORE
        .steps
        .iter()
        .position(|step| step.name == STEP)
        .unwrap();
    let before = kroma_sqlite::Schema {
        steps: &CORE.steps[..at],
        ..CORE
    };
    let mut conn = Connection::open_in_memory().unwrap();
    kroma_sqlite::migrate(&mut conn, &before, None).unwrap();
    conn.execute_batch(
        "INSERT INTO libraries (id,name,kind,path,added_at) VALUES ('lib','L','movies','/x','t');
         INSERT INTO items (id,kind,title,container,library,added_at) VALUES ('m','movie','T','mkv','lib','t');
         INSERT INTO files (id,item_id,abs_path,container,probed,v_hdr,a_codec,audio_tracks) VALUES
           ('hdr','m','/hdr','mkv',1,1,'aac','[]'),
           ('dolby','m','/dolby','mkv',1,0,'aac','[{\"index\":0,\"codec\":\"aac\"},{\"index\":1,\"codec\":\"truehd\"}]'),
           ('dts','m','/dts','mkv',1,0,'dts','[]'),
           ('plain','m','/plain','mkv',1,0,'aac','[{\"index\":0,\"codec\":\"aac\"}]'),
           ('unprobed','m','/unprobed','mkv',0,NULL,NULL,'[]'),
           ('broken','m','/broken','mkv',1,1,NULL,'[]');
         UPDATE files SET unreadable = 'moov atom not found' WHERE id = 'broken';",
    )
    .unwrap();

    kroma_sqlite::migrate(&mut conn, &CORE, None).unwrap();

    let ids = |sql: &str| -> BTreeSet<String> {
        conn.prepare(sql)
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    let named = |names: &[&str]| -> BTreeSet<String> { names.iter().map(|n| n.to_string()).collect() };
    assert_eq!(
        ids("SELECT id FROM files WHERE probed = 1 AND unreadable IS NULL AND probe_schema = 0"),
        named(&["dolby", "dts", "hdr"]),
        "the files that can learn something are waiting for a re-read"
    );
    assert_eq!(
        ids("SELECT id FROM files WHERE probed = 0"),
        named(&["unprobed"]),
        "no file was un-probed by the step"
    );
    assert_eq!(
        ids("SELECT id FROM files WHERE probed = 1"),
        named(&["broken", "dolby", "dts", "hdr", "plain"])
    );
    let columns: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('files') WHERE name LIKE 'v_dv_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(
        ["v_dv_level", "v_dv_compat", "v_dv_el"]
            .iter()
            .all(|c| columns.iter().any(|f| f == c)),
        "{columns:?}"
    );
}
