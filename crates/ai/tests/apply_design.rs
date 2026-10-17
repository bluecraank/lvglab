//! Applies a design in exactly the format the model returns and checks the
//! resulting project.

use lvglab_ai::Design;
use lvglab_model::event::Action;
use lvglab_model::flags::{ObjState, StateMask};
use lvglab_model::style::{StyleProp, StyleValue};
use lvglab_model::{Project, WidgetKind};

fn design() -> Design {
    serde_json::from_str(include_str!("fixtures/settings_design.json")).expect("fixture matches the schema")
}

#[test]
fn applies_a_complete_design_without_warnings() {
    let mut project = Project::new("Demo");
    let applied = lvglab_ai::apply(&mut project, &design());
    assert!(applied.warnings.is_empty(), "{:?}", applied.warnings);
    assert_eq!(applied.screens.len(), 2);

    let settings = project.screens.iter().find(|s| s.name == "Settings").unwrap();
    let list = settings.children.iter().find(|w| w.name == "OptionList").unwrap();
    assert!(list.layout.is_some());
    assert_eq!(list.children.len(), 3);
    let wifi = &list.children[0].children[1];
    assert_eq!(wifi.kind, WidgetKind::Switch);
    assert!(wifi.states.contains(&ObjState::Checked));
    let checked = StateMask::DEFAULT.with(ObjState::Checked);
    assert!(wifi.styles.get("indicator", checked, StyleProp::BgColor).is_some());
    let slider = &list.children[2];
    assert_eq!(slider.prop_pair("range"), (10, 100));
    assert_eq!(slider.styles.get("main", StateMask::DEFAULT, StyleProp::BgOpa), Some(&StyleValue::Int(77)));

    let back = settings.children.iter().find(|w| w.name == "BackButton").unwrap();
    let home = project.screens.iter().find(|s| s.name == "Home").unwrap().id;
    assert!(matches!(back.events[0].action, Action::ChangeScreen { screen: Some(id), .. } if id == home));
}

#[test]
fn applying_twice_replaces_instead_of_duplicating() {
    let mut project = Project::new("Demo");
    lvglab_ai::apply(&mut project, &design());
    let count = project.all_widgets().len();
    lvglab_ai::apply(&mut project, &design());
    assert_eq!(project.all_widgets().len(), count);
}
