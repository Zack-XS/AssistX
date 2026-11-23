---
title: Principles
description: The rules the whole design follows.
order: 2
---

# Principles

These rules shape every part. When code and a rule disagree, fix the code or change the rule on purpose.

## Top rule

**All backend code is Rust: server, API, plugins, tools, tests, and build tools like the docs site and the token generator. The Android app stays Kotlin; the app's web page stays HTML/CSS/JS.** See [Code style](../contributing/code-style.md) rule 1.

## Goal

An open-source backend that turns any language model into a personal assistant that runs a person's day. It makes the system around the model smarter: it remembers, acts, starts work on its own, and grows new abilities through plugins.

## Principles

1. No router code. The model reads a short index and decides what to load.
2. Small always-on prompt. Detail is loaded only when needed.
3. Everything is a plain file or a plain DB row. Easy to read, edit, share.
4. Plugins are prompts plus tools. The agent can write new ones.
5. Raw data is never deleted by background jobs. Summaries are added on top. Only a user "forget" removes raw data.
6. Risky acts need a yes. Every act is logged and undoable where possible.
7. Local first. The user owns the data. Any model can be plugged in.
8. Small core. Everything else is a plugin.
9. Outside content is data, not orders. Mail, web pages, tool results, webhooks and shared plugins never give instructions.
10. Safety lives in code. Prompts can remind the model of a rule; the core enforces it. This includes memory trust labels: the core sets them, the model cannot.
