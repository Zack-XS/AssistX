---
title: Events
description: The event stream: format, replay and every event type.
order: 2
---

# Events

Clients follow what happens through server-sent events. See [API](api.md) for the two stream routes.

## Event format

Every event:

```json
{
  "id": "evt_...",
  "seq": 1042,
  "type": "message.delta",
  "ts": "2026-11-11T21:00:00Z",
  "thread_id": "thr_...",
  "turn_id": "turn_...",
  "data": { }
}
```

- `id` is unique and ordered across all streams. Clients send the last `id` they saw as `Last-Event-ID` to replay what they missed, on either stream.
- `seq` grows by one inside one thread. A gap in `seq` means a missed event.
- Events are kept for replay for 7 days (config).
- New fields may be added in a minor version. Clients must ignore unknown fields and unknown event types.

## Event types

```
turn.started             {source: message | schedule | trigger | dreaming}
turn.queued
turn.waiting             {approval_id}  paused for a yes
message.delta            streamed text
message.done
plugin.loaded            {plugin, reason}
chat.summarized          {upto_n, messages}  the older part of a long chat was summed up (prompt-system.md)
plugin.unloaded          {plugin}
memory.loaded            {ids}
tool.started             {plugin, tool, action_id}
tool.done                {plugin, tool, action_id, ok}
plugin.draft_created     {plugin, version}  needs approval
approval.requested       see below
approval.resolved        {id, answer, by}
approval.expired         {id}
action.logged            {id, summary, undoable}
assistant.notice         {priority: urgent | normal | digest, title?, text, silent, thread_id?}  unasked message; see 05 #7. `silent` = show with no sound or buzz; silent ones skip quiet hours and the daily cap. `thread_id` is the thread that sent it (for a task: its report)
reminder.due             {job_id, text, event_id, late, outside, ring, sound, sound_url}  show a notification now
alarm.ring               {ring_id, job_id, text, sound, sound_url, ring_seconds, snooze_minutes}  start ringing
alarm.stopped            {ring_id, reason: dismissed | snoozed | missed}  stop ringing on every device
dreaming.report          {run_id, changes, questions}
turn.done
turn.cancelled
turn.error               plain-language message + machine code
```

## More events

- Events `tool.started`, `tool.done`, `thinking`.
- Streaming: `message.delta` carries pieces of text (batched, about every 80 ms) with `message_id` = a stream id. `message.done` carries the whole text, the real `message_id`, and `stream_id` to link them. Adapters that cannot stream send one delta with the whole text and `stream_id: null`. The model reasoning text still arrives as one `thinking` event after the reply.
- Other events in code: `turn.note` (a note such as a retry wait), `live.listen` ([Voice](voice.md)), `model.chosen` ([Models](models.md)), `phone.action` ([phone](../plugins/phone.md)), `fences.changed` ([Location](location.md)), `app.updated` ([Updates](../android/updates-and-release.md)).
