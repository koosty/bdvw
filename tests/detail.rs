//! Seam S1: parse_detail over fixture JSON captured from
//! `bd show <id> --json --long --include-comments --include-dependents`.
//! Expected values hand-labeled from the captured workspace.

use bdui::model::{parse_detail, DepLink};

/// Fixture: the "Beta blocked task" record captured from a real workspace.
/// It depends on Alpha (blocks), has no comments.
fn beta_fixture() -> &'static str {
    r#"[
      {
        "id":"drawer-test-k9t","title":"Beta blocked task",
        "status":"open","priority":2,"issue_type":"task",
        "created_at":"2026-11-16T03:42:02Z","updated_at":"2026-11-16T03:42:02Z",
        "dependencies":[
          {"id":"drawer-test-05x","title":"Alpha task","notes":"note here",
           "status":"open","priority":2,"issue_type":"task","assignee":"koosty",
           "created_at":"2026-11-16T03:42:02Z","updated_at":"2026-11-16T03:42:03Z",
           "dependency_type":"blocks"}
        ],
        "dependent_count":0,"dependency_count":1,"comment_count":0,
        "revision":"7548904504217530778"
      }
    ]"#
}

/// Fixture: the "Alpha task" record. Blocked-by nothing; one dependent
/// (Beta) with dependency_type blocks; one comment by koosty.
fn alpha_fixture() -> &'static str {
    r#"[
      {
        "id":"drawer-test-05x","title":"Alpha task","notes":"note here",
        "status":"open","priority":2,"issue_type":"task","assignee":"koosty",
        "created_at":"2026-11-16T03:42:02Z","updated_at":"2026-11-16T03:42:03Z",
        "dependents":[
          {"id":"drawer-test-k9t","title":"Beta blocked task",
           "status":"open","priority":2,"issue_type":"task",
           "created_at":"0001-01-01T00:00:00Z","updated_at":"0001-01-01T00:00:00Z",
           "dependency_type":"blocks"}
        ],
        "comments":[
          {"id":"c9aab2ef-e8b6-5320-9a83-c5e789d7f0e5","issue_id":"drawer-test-05x",
           "author":"koosty","text":"first comment","created_at":"2026-11-16T03:42:03Z"}
        ],
        "dependent_count":1,"dependency_count":0,"comment_count":1,
        "revision":"-9050501928568695662"
      }
    ]"#
}

#[test]
fn beta_parses_with_blocker() {
    let detail = parse_detail(beta_fixture()).expect("fixture parses");
    assert_eq!(detail.id, "drawer-test-k9t");
    assert_eq!(detail.dependencies.len(), 1);
    let dep = &detail.dependencies[0];
    assert_eq!(dep.id, "drawer-test-05x");
    assert_eq!(dep.dependency_type, "blocks");
    assert!(detail.comments.is_empty());
    assert!(detail.dependents.is_empty());
}

#[test]
fn alpha_parses_with_dependent_and_comment() {
    let detail = parse_detail(alpha_fixture()).expect("fixture parses");
    assert_eq!(detail.id, "drawer-test-05x");
    assert_eq!(detail.dependents.len(), 1);
    assert_eq!(detail.dependents[0].id, "drawer-test-k9t");
    assert_eq!(detail.dependencies.len(), 0);
    assert_eq!(detail.comments.len(), 1);
    let c = &detail.comments[0];
    assert_eq!(c.author, "koosty");
    assert_eq!(c.text, "first comment");
}

#[test]
fn dep_type_defaults_to_blocks_when_missing() {
    let detail = parse_detail(beta_fixture()).expect("fixture parses");
    assert_eq!(
        detail.dependencies[0].dependency_type,
        DepLink::BLOCKS.to_string()
    );
}

#[test]
fn full_record_fields_survive_parse() {
    let detail = parse_detail(alpha_fixture()).expect("fixture parses");
    // Notes and assignee present on the record itself.
    assert_eq!(detail.notes.as_deref(), Some("note here"));
    assert_eq!(detail.assignee.as_deref(), Some("koosty"));
}
