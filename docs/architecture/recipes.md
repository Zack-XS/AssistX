---
title: Recipes
description: Saved multi-step tool runs the user can replay.
order: 6
---

# Recipes

`src/recipes.rs`. A recipe is a name, trigger words and ordered steps `{plugin, tool, args}`, in table `recipes`.

- Offer: when an attended normal-mode turn (not live, not a recipe run) ends well with 3 or more successful plugin calls (memory-tools and plugin-maker calls do not count), the last call worked and no card is still waiting, the core raises one low-risk `question` card "Save this as a recipe? ..." with options Save / No thanks.

  Typed words are the new name. Nothing is saved without that answer. The same steps are never offered twice.
- Placeholders: an argument value the user typed in the request becomes `{{key}}` (key = argument name), with the value as its default in `inputs`.
- Outside: a recipe whose argument values appear in outside content of the turn and not in the user's own words is marked `outside`. Each of its steps asks for a yes even when the tool is `auto`, so it never runs by itself. (Taint by argument, not by any outside text in the turn: most multi-step turns read some tool result, and only the arguments get replayed.)
- Index line `- recipes: Name, ...`. Core tools `list_recipes`, `run_recipe {name, inputs}` (each step through the normal gate, stops at the first step that does not finish), `forget_recipe` (refused in a turn with outside content).
- Routes `GET /v1/recipes`, `DELETE /v1/recipes/{id}`. The app's Settings has a Recipes page to list and delete them.

- `src/recipes.rs`: after an attended turn with 3+ successful plugin calls, one low-risk `question` card offers to save the steps as a recipe (typed values become `{{placeholders}}`). Core tools `list_recipes`, `run_recipe {name, inputs}`, `forget_recipe`; index line `recipes`; Settings > Recipes in the page. An `outside` recipe asks before every step.
