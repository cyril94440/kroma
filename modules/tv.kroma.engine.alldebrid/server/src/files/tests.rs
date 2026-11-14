use super::*;

fn file(name: &str, size: u64) -> Node {
    Node {
        n: name.into(),
        s: Some(size),
        l: Some(format!("https://alldebrid.com/f/{name}")),
        e: None,
    }
}

fn folder(name: &str, children: Vec<Node>) -> Node {
    Node {
        n: name.into(),
        s: None,
        l: None,
        e: Some(children),
    }
}

fn rels(tree: &[Node]) -> Vec<String> {
    flatten(tree).unwrap().into_iter().map(|p| p.rel).collect()
}

#[test]
fn nested_folders_become_slash_joined_relative_paths() {
    let tree = [folder(
        "Show.S01",
        vec![file("a.mkv", 1), folder("Subs", vec![file("a.srt", 2)])],
    )];

    assert_eq!(rels(&tree), ["Show.S01/a.mkv", "Show.S01/Subs/a.srt"]);
}

#[test]
fn a_file_keeps_its_size_and_link() {
    let planned = flatten(&[file("a.mkv", 42)]).unwrap();

    assert_eq!(planned[0].size, 42);
    assert_eq!(planned[0].link, "https://alldebrid.com/f/a.mkv");
}

#[test]
fn a_parent_directory_component_cannot_climb_out() {
    let tree = [folder("..", vec![folder("..", vec![file("..", 1)])])];

    assert_eq!(rels(&tree), ["_/_/_"]);
}

#[test]
fn separators_and_absolute_names_are_neutralised() {
    let tree = [
        file("/etc/passwd", 1),
        file("..\\..\\boot.ini", 1),
        file("a/../b", 1),
    ];

    assert_eq!(rels(&tree), ["_etc_passwd", ".._.._boot.ini", "a_.._b"]);
}

#[test]
fn no_planned_path_has_a_climbing_or_rooted_component() {
    let tree = [folder(
        "../x",
        vec![file("/abs", 1), file("....", 1), file("", 1)],
    )];

    for rel in rels(&tree) {
        assert!(!rel.starts_with('/'), "{rel}");
        assert!(
            rel.split('/')
                .all(|part| !part.is_empty() && part.chars().any(|c| c != '.')),
            "{rel}"
        );
    }
}

#[test]
fn control_characters_and_nul_are_replaced() {
    assert_eq!(component("a\0b\nc"), "a_b_c");
}

#[test]
fn colliding_names_are_kept_apart() {
    let tree = [file("a.mkv", 1), file("A.mkv", 2), file("a.mkv", 3)];

    assert_eq!(rels(&tree), ["a.mkv", "A (2).mkv", "a (3).mkv"]);
}

#[test]
fn a_long_name_is_cut_on_a_character_boundary() {
    let name = "é".repeat(300);

    let cut = component(&name);

    assert!(cut.len() <= MAX_NAME_BYTES);
    assert!(cut.chars().all(|c| c == 'é'));
}

#[test]
fn an_entry_without_a_link_is_skipped() {
    let bare = Node {
        n: "x".into(),
        s: Some(1),
        l: None,
        e: None,
    };

    assert_eq!(rels(&[bare, file("a.mkv", 1)]), ["a.mkv"]);
}

#[test]
fn a_magnet_with_no_file_is_an_error() {
    assert!(flatten(&[folder("empty", vec![])]).is_err());
}

#[test]
fn a_tree_nested_past_the_limit_is_refused() {
    let mut tree = vec![file("a.mkv", 1)];
    for _ in 0..=MAX_DEPTH {
        tree = vec![folder("d", tree)];
    }

    assert!(flatten(&tree).is_err());
}

#[test]
fn files_are_told_apart_by_what_the_library_will_do_with_them() {
    assert_eq!(kind_of("a/b.MKV"), Kind::Video);
    assert_eq!(kind_of("b.srt"), Kind::Subtitle);
    assert_eq!(kind_of("b.WebVTT"), Kind::Subtitle);
    assert_eq!(kind_of("b.sub"), Kind::Other);
    assert_eq!(kind_of("b.nfo"), Kind::Other);
    assert_eq!(kind_of("mkv"), Kind::Other);
}
