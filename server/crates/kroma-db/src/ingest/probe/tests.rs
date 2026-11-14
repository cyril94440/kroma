use super::*;
use crate::ingest::test_support::*;
use std::collections::HashMap;

#[test]
fn probe_helpers_over_seeded_files() {
    let p = pool();
    {
        let conn = p.get().unwrap();
        conn.execute(
            "INSERT INTO libraries (id,name,kind,path,added_at) VALUES ('lib','L','movies','/x','t')",
            [],
        )
        .unwrap();
        for id in ["m1", "m2"] {
            conn.execute(
                "INSERT INTO items (id,kind,title,container,library,added_at) VALUES (?1,'movie','T','mkv','lib','t')",
                params![id],
            )
            .unwrap();
        }
        // m1: one probed + one unprobed (not fully probed).
        conn.execute(
            "INSERT INTO files (id,item_id,abs_path,container,probed,mtime,size) VALUES ('f1','m1','/a',' ',1,100,2000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files (id,item_id,abs_path,container,probed) VALUES ('f2','m1','/b','',0)",
            [],
        )
        .unwrap();
        // m2: a single probed file (fully probed).
        conn.execute(
            "INSERT INTO files (id,item_id,abs_path,container,probed) VALUES ('f3','m2','/c','',1)",
            [],
        )
        .unwrap();
    }

    assert!(!item_probed(&p, "m1").unwrap());
    assert!(item_probed(&p, "m2").unwrap());
    assert!(!item_probed(&p, "missing").unwrap());

    let fully = probed_item_ids(&p).unwrap();
    assert!(fully.contains("m2") && !fully.contains("m1"));

    let mut ids = file_ids_for_item(&p, "m1").unwrap();
    ids.sort();
    assert_eq!(ids, vec!["f1".to_string(), "f2".to_string()]);

    let f1 = probe_target(&p, "f1").unwrap().expect("the probed file");
    assert_eq!((f1.abs_path.as_str(), f1.item_id.as_str()), ("/a", "m1"));
    assert!(f1.described);
    assert!(probe_target(&p, "gone").unwrap().is_none());
    let sigs: HashMap<String, String> = all_file_sigs(&p).unwrap().into_iter().collect();
    assert_eq!(sigs.get("f1").map(String::as_str), Some("100:2000"));
    assert_eq!(sigs.get("f3").map(String::as_str), Some("0:0")); // null mtime/size

    unprobe_item_files(&p, "m1").unwrap();
    assert!(!item_has_probed_file(&p, "m1").unwrap());
}

#[test]
fn a_file_a_pass_left_with_a_fault_is_offered_to_the_next_one() {
    let p = pool();
    crate::sync_all(
        &p,
        &[lib("lib")],
        &[],
        &[movie(
            "m1",
            "Dune",
            "lib",
            vec![file("f1", "/media/m1.mkv", false)],
        )],
        &HashMap::new(),
    )
    .unwrap();
    crate::set_file_probe(
        &p,
        "f1",
        &crate::FileProbe {
            unreadable: Some("Invalid data found when processing input"),
            ..crate::FileProbe::default()
        },
    )
    .unwrap();

    let target = probe_target(&p, "f1").unwrap().expect("the faulted file");

    assert!(!target.described);
}

#[test]
fn the_probe_state_readers_error_when_the_file_table_is_gone() {
    let p = pool();
    p.get().unwrap().execute_batch("DROP TABLE files").unwrap();

    assert!(item_probed(&p, "m1").is_err());
    assert!(item_has_probed_file(&p, "m1").is_err());
}

fn seeded_library() -> crate::testing::TempPool {
    let p = pool();
    sync_all_files(
        &p,
        vec![
            file("sdr", "/media/sdr.mkv", true),
            file("hdr", "/media/hdr.mkv", true),
            file("fresh", "/media/fresh.mkv", false),
        ],
    );
    let conn = p.get().unwrap();
    conn.execute_batch(
        "UPDATE files SET probe_schema = 0 WHERE id IN ('sdr', 'hdr');
         UPDATE files SET v_hdr = 1 WHERE id = 'hdr';",
    )
    .unwrap();
    drop(conn);
    p
}

fn sync_all_files(p: &crate::Pool, files: Vec<kroma_domain::MediaFile>) {
    crate::sync_all(
        p,
        &[lib("lib")],
        &[],
        &[movie("m1", "Dune", "lib", files)],
        &HashMap::new(),
    )
    .unwrap();
}

#[test]
fn a_file_an_older_probe_read_is_still_probed_for_every_reader() {
    let p = seeded_library();

    assert!(item_has_probed_file(&p, "m1").unwrap());
    assert!(probed_item_ids(&p).unwrap().is_empty(), "fresh is unprobed");
    let probed: i64 = p
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM files WHERE probed = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(probed, 2);
    assert_eq!(unprobed_files(&p).unwrap().len(), 1);
}

#[test]
fn the_stale_files_are_listed_hdr_first_and_never_include_an_unprobed_or_unreadable_one() {
    let p = seeded_library();
    p.get()
        .unwrap()
        .execute_batch(
            "INSERT INTO files (id,item_id,abs_path,container,probed,probe_schema,unreadable) \
             VALUES ('broken','m1','/media/broken.mkv','mkv',1,0,'moov atom not found')",
        )
        .unwrap();

    let stale = stale_probe_files(&p, 10).unwrap();

    let ids: Vec<&str> = stale.iter().map(|(id, _, _)| id.as_str()).collect();
    assert_eq!(ids, ["hdr", "sdr"]);
    assert_eq!(stale_probe_count(&p).unwrap(), 2);
}

#[test]
fn a_batch_is_bounded() {
    let p = seeded_library();

    assert_eq!(stale_probe_files(&p, 1).unwrap().len(), 1);
    assert!(stale_probe_files(&p, 0).unwrap().is_empty());
}

#[test]
fn a_re_probe_replaces_the_facts_and_settles_the_file_in_one_write() {
    let p = seeded_library();

    crate::set_file_probe(
        &p,
        "hdr",
        &crate::FileProbe {
            duration_ms: Some(42),
            video: Some(&video()),
            ..crate::FileProbe::default()
        },
    )
    .unwrap();

    let (duration, schema): (i64, i64) = p
        .get()
        .unwrap()
        .query_row(
            "SELECT duration_ms, probe_schema FROM files WHERE id = 'hdr'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((duration, schema), (42, PROBE_SCHEMA));
    assert_eq!(stale_probe_count(&p).unwrap(), 1);
}

#[test]
fn a_failed_re_read_keeps_the_old_facts_and_is_not_offered_again() {
    let p = seeded_library();

    keep_probe_facts(&p, "hdr").unwrap();

    let (duration, hdr): (i64, i64) = p
        .get()
        .unwrap()
        .query_row(
            "SELECT duration_ms, v_hdr FROM files WHERE id = 'hdr'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((duration, hdr), (7_200_000, 1));
    assert_eq!(stale_probe_count(&p).unwrap(), 1);
    assert!(stale_probe_files(&p, 10)
        .unwrap()
        .iter()
        .all(|(id, _, _)| id != "hdr"));
}

#[test]
fn a_file_the_scan_hands_over_already_probed_is_current() {
    let p = pool();
    sync_all_files(&p, vec![file("f1", "/media/f1.mkv", true)]);

    assert_eq!(stale_probe_count(&p).unwrap(), 0);
}

#[test]
fn a_file_that_changed_on_disk_is_unprobed_again_and_current_once_it_is_read() {
    let p = seeded_library();
    crate::set_file_probe(&p, "fresh", &crate::FileProbe::default()).unwrap();

    let schema: i64 = p
        .get()
        .unwrap()
        .query_row("SELECT probe_schema FROM files WHERE id = 'fresh'", [], |r| r.get(0))
        .unwrap();

    assert_eq!(schema, PROBE_SCHEMA);
}
