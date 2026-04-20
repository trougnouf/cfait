## Summary

- Port the `cfait2` TUI create-mode fix so `#tag` and `@@location` smart-input entries create tasks instead of unexpectedly switching sidebar filters.
- Add `enable_local_mode = true` to the config, allowing the TUI to hide local/offline calendars and require remote targets for new tasks when set to `false`.
- Preserve already loaded calendar tasks inside the TUI network actor when creating, updating, deleting, moving, or refreshing tasks so cached tasks are not dropped from the in-memory snapshot.

## Details

- `src/config.rs`
  - Added the new `enable_local_mode` config field with a default of `true`.
  - Documented the setting in generated `config.toml` comments.
- `src/tui/state.rs`
  - Track whether local mode is enabled.
  - Filter local calendars and local task buckets out of the derived TUI view when disabled.
- `src/tui/handlers.rs`
  - Stop treating tag-only or location-only input as sidebar filter commands while in create mode.
  - When local mode is disabled, exclude local calendars from loaded state and avoid selecting them as creation targets.
  - Show a clearer message if no remote calendar is available for task creation.
- `src/tui/network.rs`
  - Thread `enable_local_mode` into the network actor.
  - Filter local calendars out of actor discovery when disabled.
  - Keep the actor's internal store in sync with cached and fetched task snapshots so later CRUD refreshes do not lose previously loaded tasks.
- `tests/tui_create_input.rs`
  - Covers the create-mode `#tag` regression.
- `tests/tui_local_mode.rs`
  - Covers local-calendar filtering when `enable_local_mode = false`.
- `tests/tui_network_create.rs`
  - Covers preserving existing cached tasks after creating a new remote task.
- `tests/tui_network_toggle.rs`
  - Updated for the new `NetworkActorConfig` field.

## Suggested PR Notes

This ports the local fixes from `cfait2` onto current `master` without pulling in the fork's older upstream state. The functional changes are:

1. TUI create mode now consistently creates tasks from smart input like `#work` or `@@office` instead of hijacking the sidebar filter.
2. A new `enable_local_mode` config flag lets users hide local/offline calendars in the TUI and require remote calendars for new tasks.
3. The TUI network actor now keeps previously loaded tasks in its internal store, fixing a regression where creating a task could replace an existing cached calendar snapshot with only the new task.

Feature parity note: `enable_local_mode` is intentionally TUI-only in this patch. GUI and Android behavior are unchanged.

## Verification

Run:

```bash
cargo test --test tui_create_input --test tui_local_mode --test tui_network_create --test tui_network_toggle
```
