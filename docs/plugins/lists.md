---
title: lists
description: The user's lists - shopping, to-do or any name; add, tick off, remove and share items.
order: 8
---

# lists

The user's lists - shopping, to-do or any name; add, tick off, remove and share items.

- Backend: `core` (src/lists.rs; lists and items live in the database)
- Needs: nothing outside the core
- Quick-command words: ["shopping list", "to-do", "todo", "to do list", "to-do list", "grocery", "groceries", "my list", "add to my list", "on my list", "checklist", "lists"]
- Guide for the model: [guide.md](../../server/plugins/lists/guide.md) (runtime file, loaded by the engine)
- Manifest: [plugin.yaml](../../server/plugins/lists/plugin.yaml)

## Tools

| Tool | Tags | Asks first |
|---|---|---|
| `list_lists` | reads | no |
| `show_list` | reads | no |
| `add_items` | writes | no |
| `undo_add_items` | writes | no |
| `check_items` | writes | no |
| `uncheck_items` | writes | no |
| `remove_items` | writes | no |
| `clear_checked` | writes | no |
| `restore_items` | writes | no |
| `rename_list` | writes | no |
| `delete_list` | writes, deletes | yes |
| `restore_list` | writes | no |

Asks first comes from `autonomy` in `plugin.yaml`. The core can raise it (see [Approvals](../agents/approvals.md)).

## Limits

Shopping, to-do or any name; items with quantity, note, due date. Names match loosely (groceries = shopping). Lists with open items shown in the index as counts. App: Plan, Lists (tick, add, share).

Shopping, to-do and any named list, kept in the database (`src/lists.rs`, tables `lists` and `list_items`). Tools, all `auto` but `delete_list` (`ask`):

`list_lists`, `show_list`, `add_items` (many at once; makes the list if new), `check_items`, `uncheck_items`, `remove_items`, `clear_checked`, `rename_list`, `delete_list`.

Every write has an undo (`undo_add_items`, `uncheck_items`/`check_items`, `restore_items`, `restore_list`). Items added in a turn that read outside content are marked `outside`.

Quick-command words (`shopping list`, `to-do`, `groceries`, `my list`, ...) load it at once. The index shows each list with open items as a name and a count, wrapped as outside content. Routes in [API](../architecture/api.md), Lists.

## Routes

```
GET    /v1/lists                            lists with open and done counts
POST   /v1/lists                            make one: name, items?                  201
GET    /v1/lists/{id}                       the list and its items (open first, then done)
PATCH  /v1/lists/{id}                       rename: name
DELETE /v1/lists/{id}                       delete the list (items kept with it for undo)
POST   /v1/lists/{id}/items                 add: text, quantity?, note?, due?  or  items: [...]   201
PATCH  /v1/lists/{id}/items/{item}          done?, text?, quantity?, note?, due? ("" clears)
DELETE /v1/lists/{id}/items/{item}          remove one item
POST   /v1/lists/{id}/clear                 remove the ticked items
GET    /v1/lists/{id}/text                  the list as plain text, for sharing (text/plain)
```

A list is found by id or by name: `shopping list`, `Shopping` and `groceries` are one list, as are `todo`, `to do` and `to-do`. Names are unique. Items carry an optional `quantity`, `note` and `due` (`YYYY-MM-DD` or `YYYY-MM-DD HH:MM`).

The same open words are not added twice; adding a ticked item opens it again. Limits: 100 lists, 500 items per list, 100 items per call.

The model uses the `lists` plugin ([Plugins](../architecture/plugins.md)); lists with open items appear in the index as names and counts only, wrapped as outside content.

Sharing is the client's job: the Android app passes the plain text to the phone's share sheet.
