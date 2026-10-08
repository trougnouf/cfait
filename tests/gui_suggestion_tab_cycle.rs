// SPDX-License-Identifier: GPL-3.0-or-later
//! Tab in the add-task input must apply the top autocomplete suggestion and
//! repeated presses must cycle through the rest of the list, without falling
//! back to focus cycling while a suggestion is available.
#![cfg(feature = "gui")]

use cfait::gui::message::Message;
use cfait::gui::state::GuiApp;
use cfait::gui::update;
use cfait::model::Task;
use std::collections::HashMap;

fn app_with_tags(tags: &[&str]) -> GuiApp {
    let mut app = GuiApp::default();
    for (i, tag) in tags.iter().enumerate() {
        let mut t = Task::new(&format!("prune the roses #{tag}"), &HashMap::new(), None);
        t.uid = format!("uid-{i}");
        t.calendar_href = "local://default".to_string();
        app.store.add_task(t);
    }
    app
}

fn type_text(app: &mut GuiApp, text: &str) {
    for c in text.chars() {
        let _ = update::update(
            app,
            Message::InputChanged(iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert(c),
            )),
        );
    }
}

fn press_tab(app: &mut GuiApp, forward: bool) {
    let _ = update::update(app, Message::TabPressed(forward));
}

#[test]
fn tab_applies_then_cycles_tag_suggestions() {
    let mut app = app_with_tags(&["gardening", "gardenias", "garlic"]);

    type_text(&mut app, "water the beds #gar");
    assert_eq!(app.input_value.text(), "water the beds #gar");

    let text = app.input_value.text();
    let direct = cfait::model::autocomplete::suggest(
        &text,
        text.len(),
        &app.store,
        &app.tag_aliases,
        &app.calendars,
        &app.visible_calendar_hrefs(),
    );
    assert_eq!(
        direct.map(|(range, suggs)| {
            (
                range,
                suggs
                    .iter()
                    .map(|s| s.replacement.clone())
                    .collect::<Vec<_>>(),
            )
        }),
        Some((
            15..19,
            vec![
                "#gardenias".to_string(),
                "#gardening".to_string(),
                "#garlic".to_string(),
            ]
        )),
        "the banner should show all three tags for the #gar prefix"
    );

    press_tab(&mut app, true);
    assert_eq!(
        app.input_value.text(),
        "water the beds #gardenias ",
        "first Tab should apply the top suggestion"
    );

    press_tab(&mut app, true);
    assert_eq!(
        app.input_value.text(),
        "water the beds #gardening ",
        "second Tab should cycle to the next suggestion"
    );

    press_tab(&mut app, true);
    assert_eq!(
        app.input_value.text(),
        "water the beds #garlic ",
        "third Tab should cycle to the last suggestion"
    );

    press_tab(&mut app, true);
    assert_eq!(
        app.input_value.text(),
        "water the beds #gardenias ",
        "the cycle should wrap around to the first suggestion"
    );

    press_tab(&mut app, false);
    assert_eq!(
        app.input_value.text(),
        "water the beds #garlic ",
        "Shift+Tab should cycle backwards"
    );
}

#[test]
fn tab_without_suggestion_leaves_text_untouched() {
    let mut app = app_with_tags(&["gardening"]);

    type_text(&mut app, "no tag prefix here");
    press_tab(&mut app, true);
    assert_eq!(
        app.input_value.text(),
        "no tag prefix here",
        "Tab without a suggestion must not edit the text"
    );
}
