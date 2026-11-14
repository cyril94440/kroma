use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::infra::events::Bus;

fn sources() -> Arc<Sources> {
    Sources::new(
        Arc::new(|_| Vec::new()),
        Arc::new(Vec::new),
        Arc::new(|_, _| true),
        Arc::new(|_| Vec::new()),
    )
}

fn clip(dir: &Path) -> Option<std::path::PathBuf> {
    let path = dir.join("clip.mp4");
    let mut cmd = std::process::Command::new("ffmpeg");
    kroma_media::input::without_proxies(&mut cmd);
    cmd.args(["-v", "error"])
        .args(kroma_media::input::generated(
            "testsrc=duration=1:size=64x48:rate=10",
        ))
        .args(["-c:v", "libx264", "-y"])
        .arg(&path)
        .status()
        .ok()?
        .success()
        .then_some(path)
}

fn library(pool: &Pool, files: &[(&str, &Path)]) {
    let conn = pool.get().unwrap();
    conn.execute_batch(
        "INSERT INTO libraries (id,name,kind,path,added_at) VALUES ('lib','L','movies','/x','t');
         INSERT INTO items (id,kind,title,container,library,added_at) VALUES ('m','movie','T','mkv','lib','t');",
    )
    .unwrap();
    for (id, path) in files {
        conn.execute_batch(&format!(
            "INSERT INTO files (id,item_id,abs_path,container,probed,probe_schema,v_codec,v_hdr,duration_ms) \
             VALUES ('{id}','m','{}','mp4',1,0,'old',0,7)",
            path.display()
        ))
        .unwrap();
    }
}

fn facts(pool: &Pool, id: &str) -> (i64, Option<String>, i64) {
    pool.get()
        .unwrap()
        .query_row(
            &format!("SELECT probed, v_codec, probe_schema FROM files WHERE id = '{id}'"),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

#[test]
fn a_stale_file_is_read_again_and_swapped_to_its_new_facts_in_one_write() {
    let dir = kroma_testing::temp_dir("refresh-one");
    let Some(clip) = clip(dir.path()) else { return };
    let pool = kroma_db::testing::temp_pool("refresh-one");
    library(&pool, &[("f1", &clip)]);
    assert_eq!(facts(&pool, "f1"), (1, Some("old".into()), 0));

    refresh_stale(&pool, true, &sources(), &Bus::new());

    let (probed, codec, schema) = facts(&pool, "f1");
    assert_eq!(
        (probed, codec.as_deref(), schema),
        (1, Some("h264"), kroma_db::PROBE_SCHEMA)
    );
    assert_eq!(db::stale_probe_count(&pool).unwrap(), 0);
}

#[test]
fn a_file_that_cannot_be_read_again_keeps_its_facts_and_is_not_offered_again() {
    let pool = kroma_db::testing::temp_pool("refresh-gone");
    library(&pool, &[("gone", Path::new("/nonexistent/clip.mp4"))]);

    refresh_stale(&pool, true, &sources(), &Bus::new());

    assert_eq!(
        facts(&pool, "gone"),
        (1, Some("old".into()), kroma_db::PROBE_SCHEMA)
    );
}

#[test]
fn nothing_is_read_again_without_ffprobe() {
    let pool = kroma_db::testing::temp_pool("refresh-no-ffprobe");
    library(&pool, &[("f1", Path::new("/nonexistent/clip.mp4"))]);

    refresh_stale(&pool, false, &sources(), &Bus::new());

    assert_eq!(facts(&pool, "f1"), (1, Some("old".into()), 0));
}

#[test]
fn a_pass_takes_a_bounded_batch_and_leaves_the_rest_probed_and_waiting() {
    let pool = kroma_db::testing::temp_pool("refresh-batch");
    let gone = Path::new("/nonexistent/clip.mp4");
    let ids: Vec<String> = (0..BATCH + 5).map(|n| format!("f{n:04}")).collect();
    {
        let conn = pool.get().unwrap();
        conn.execute_batch(
            "INSERT INTO libraries (id,name,kind,path,added_at) VALUES ('lib','L','movies','/x','t');
             INSERT INTO items (id,kind,title,container,library,added_at) VALUES ('m','movie','T','mkv','lib','t');",
        )
        .unwrap();
        for id in &ids {
            conn.execute_batch(&format!(
                "INSERT INTO files (id,item_id,abs_path,container,probed,probe_schema) VALUES ('{id}','m','{}/{id}','mkv',1,0)",
                gone.display()
            ))
            .unwrap();
        }
    }

    assert_eq!(db::stale_probe_files(&pool, BATCH).unwrap().len(), BATCH);
    assert_eq!(db::stale_probe_count(&pool).unwrap(), BATCH + 5);
    let unprobed: i64 = pool
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM files WHERE probed = 0", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(unprobed, 0);
}

#[test]
fn a_second_pass_asked_for_while_one_runs_does_nothing() {
    let first = Running::claim();

    assert!(first.is_some());
    assert!(Running::claim().is_none());
    drop(first);
    assert!(Running::claim().is_some());
}
