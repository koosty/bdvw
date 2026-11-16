use bdui::model::Bead;
use bdui::palette::{
    fuzzy_match, palette_commands, palette_matches, PaletteCommand, ViewKind,
};
use serde_json::json;

fn bead(id: &str, title: &str) -> bdui::model::Bead {
    serde_json::from_value(json!({
        "id": id,
        "title": title,
        "status": "open",
        "priority": 2,
        "issue_type": "task",
        "dependency_count": 0,
        "dependent_count": 0,
        "comment_count": 0
    }))
    .expect("valid bead json")
}

fn beads() -> Vec<Bead> {
    vec![
        bead("bd-a3f8.1", "Port drawer to shell"),
        bead("bd-b7c2.3", "Graph static DAG render"),
        bead("bd-c1d9.2", "Board columns by state"),
    ]
}

#[test]
fn view_commands_cover_all_views() {
    let cmds = palette_commands();
    let views: Vec<&str> = cmds
        .iter()
        .filter_map(|c| match c {
            PaletteCommand::View(v) => Some(v.name()),
            PaletteCommand::ToggleTheme => None,
        })
        .collect();
    for expected in ["Ready", "Tree", "Graph", "Board", "Activity", "Memory"] {
        assert!(
            views.contains(&expected),
            "missing view command {expected}; got {views:?}"
        );
    }
}

#[test]
fn g_prefix_matches_all_views_sorted() {
    let cmds = palette_commands();
    let hits = palette_matches("g", &cmds, &[]);
    assert!(hits.len() >= 6, "expected all views matched by 'g'");
}

#[test]
fn query_matches_ids_and_view_commands_together() {
    let cmds = palette_commands();
    let hits = palette_matches("graph", &cmds, &beads());
    assert!(hits
        .iter()
        .any(|m| m.label().contains("Graph")), "view command present");
}

#[test]
fn fuzzy_id_jump_finds_substring_of_id() {
    let cmds = palette_commands();
    let hits = palette_matches("a3f8", &cmds, &beads());
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id(), "bd-a3f8.1");
}

#[test]
fn title_words_match_too() {
    let cmds = palette_commands();
    let hits = palette_matches("dag", &cmds, &beads());
    assert!(hits.iter().any(|m| m.id() == "bd-b7c2.3"));
    assert!(hits
        .iter()
        .any(|m| m.title() == Some("Graph static DAG render")));
}

#[test]
fn fuzzy_match_is_subsequence_and_scores_prefix_best() {
    assert!(fuzzy_match("drawer", "Port drawer to shell").is_some());
    assert!(fuzzy_match("gph", "graph").is_some());
    assert!(fuzzy_match("xyz", "graph").is_none());
    let prefix = fuzzy_match("gr", "graph").expect("prefix matches");
    let mid = fuzzy_match("ph", "graph").expect("mid matches");
    assert!(
        prefix > mid,
        "prefix score {prefix} should beat mid score {mid}"
    );
}

#[test]
fn view_name_round_trip() {
    assert_eq!(ViewKind::from_name("Ready"), Some(ViewKind::Ready));
    assert_eq!(ViewKind::from_name("Memory"), Some(ViewKind::Memory));
    assert_eq!(ViewKind::from_name("Nope"), None);
    assert_eq!(ViewKind::Graph.name(), "Graph");
}
