---
title: Multi-device plan
description: A plan for one owner with many devices. Not built yet.
order: 12
---

# Multi-device plan

Status: plan, not built. See [Next](../roadmap/next.md).

One owner, many devices, one brain. Parts of it exist in other forms: per-device tokens (`device` scope), computers as nodes ([Computers](computers.md)) and remembered phones ([Accounts](accounts.md)).

## Goal

The assistant knows which devices the owner has, what each can do, and which one a request came from. It builds this from variables (env + config file), not from guessing. The model reads a short device summary; the core enforces the rules.

This is not multi-user (see [Open questions](../roadmap/open-questions.md) #8). Every device belongs to the same `owner_id`.

## Model: one host, many clients

- **Host**: the one device that runs the core, DB, scheduler and plugins. Local first: data lives here.
- **Client**: a device that talks to the host over the API with its own `client` token (front end, phone, tablet, watch).
- **Node** (later): a device that also offers tools, such as a phone camera or a home hub. Tools run on the node, called through the host.

The host is the only writer to the DB. Clients never sync a copy of memory.

## Where devices are declared

Two sources. Env overrides the file.

1. File `devices.toml` in the config dir.
2. Env variables, one group per device: `ASSISTANT_DEVICE_<ID>_<FIELD>`.

```toml
[devices.laptop]
name = "Main laptop"
kind = "laptop"            # laptop | desktop | phone | tablet | watch | server | home_hub | other
os = "macos"
role = "host"              # host | client | node
capabilities = ["files", "shell", "calendar_ui"]
can_approve = ["low", "medium", "high"]
location = "home"
token_ref = "device-laptop"  # name of a secret in the keychain, never the value

[devices.phone]
name = "Pixel phone"
kind = "phone"
os = "android"
role = "client"
capabilities = ["notify", "camera", "gps", "voice"]
can_approve = ["low", "medium"]
```

Same device in env:

```
ASSISTANT_DEVICE_PHONE_NAME="Pixel phone"
ASSISTANT_DEVICE_PHONE_KIND=phone
ASSISTANT_DEVICE_PHONE_ROLE=client
ASSISTANT_DEVICE_PHONE_CAPABILITIES=notify,camera,gps,voice
ASSISTANT_DEVICE_PHONE_CAN_APPROVE=low,medium
ASSISTANT_DEVICE_PHONE_TOKEN_REF=device-phone
```

Other variables:

| Variable | Meaning |
|---|---|
| `ASSISTANT_DEVICE_ID` | Which device this process is. Set on the host and on each node. |
| `ASSISTANT_DEVICES_FILE` | Path to a different `devices.toml`. |
| `ASSISTANT_DEVICES_STRICT` | If `true`, unknown devices are refused. Default `true`. |

Rules:

- Env and file hold **secret references only**. Token values live in the keychain.
- Bad or duplicate IDs stop startup with a clear error.
- Exactly one device has `role = "host"`.

## The device registry (core)

At start the core:

1. Reads file and env, merges them (env wins), validates the schema.
2. Stores the result in a `devices` table: `id`, `owner_id`, `name`, `kind`, `os`, `role`, `capabilities`, `can_approve`, `location`, `token_ref`, `status`, `last_seen`, `paired_at`, `revoked_at`.
3. Marks `status` from live signals: `online` (token used or SSE open in the last minutes), `idle`, `offline`.

`last_seen` and `status` come from the core, never from the device's own claims alone. Capabilities are what the owner declared; a device cannot add its own.

## How the model understands devices

Core-set, read-only for the model (same rule as memory trust labels).

- **Device index**: a few lines added to the always-on index. Example: `laptop (host, online, files+shell) | phone (client, online, notify+camera+gps) | watch (client, offline)`.
- **Current device**: each turn carries `origin_device` (which client sent the message, from its token). The prompt says "This message came from: phone".
- **Tools**: `list_devices`, `device_info(id)`. Read-only, `auto` level.
- **Memory**: entries may carry `device_id` (where a fact was learned). Facts like "I keep my passport at home" stay device-neutral.

The model uses this to adapt: short replies on a watch, a notification instead of a long text on a phone, shell work only on the laptop.

## Routing

- Tool calls may carry a `device` argument. Default is the host, or the origin device when the tool needs one (camera, GPS).
- The Safety Gate adds checks: the target device exists, is not revoked, and lists the needed capability. A tool on a device without the capability is refused with `device_capability_missing`.
- If the target is offline, the call fails with `device_offline`. Calls that may wait (notify, reminders) go into a short queue with an expiry. Nothing risky is queued.
- Notices (`assistant.notice`, see [Open questions](../roadmap/open-questions.md) #7) go to a device by rule: `urgent` to every online device that can notify, `normal` to the origin or last active one, `digest` to the chosen digest device.

## Approvals across devices

- The approval card shows the real tool arguments **and the target device**.
- An approval can be answered only from a device whose `can_approve` includes the card's risk level. The watch cannot approve `high`.
- `high` approvals use the 5-minute single-use token trade (open question in [Open questions](../roadmap/open-questions.md)), so no client stores the owner token.
- When one device answers, the card is closed on all others.

## Pairing and trust

1. Owner starts pairing on the host: `assistant devices pair --id phone`.
2. Host shows a short-lived code (5 minutes, single use) and a device fingerprint.
3. The new device sends the code. The host checks it against the declared `phone` entry, issues a `client` token, and saves it under `token_ref`.
4. Owner can list, rename or revoke: `assistant devices list | revoke <id>`. Revoke kills the token and open streams at once.

Rules:

- Pairing only works for a device already declared in the registry (strict mode).
- One token per device. A stolen phone is revoked without touching the others.
- Pairing over a non-local network needs TLS.
- Every pair, revoke and capability change goes to the action log and fires an event.

## API additions (sketch)

| Route | Use | Token |
|---|---|---|
| `GET /v1/devices` | List devices and status | client |
| `GET /v1/devices/{id}` | One device | client |
| `POST /v1/devices/pair` | Start pairing | owner |
| `POST /v1/devices/{id}/revoke` | Revoke | owner |
| `POST /v1/devices/{id}/heartbeat` | Report alive | that device |

Events: `device.online`, `device.offline`, `device.paired`, `device.revoked`.

## Threats to cover (add to [Threat model](../security/threat-model.md))

| Threat | Defense |
|---|---|
| Stolen or lost device | Per-device token, instant revoke, `can_approve` limits |
| Fake device joins | Strict registry, single-use pairing code, fingerprint check |
| Device claims power it lacks | Capabilities set by owner config, not by the device |
| Model changes device list | Registry is core-only; model tools are read-only |
| Approval shown on weak device | `can_approve` per risk level; card shows target device |
| Env leaks tokens | Env holds `token_ref` only; values in keychain; scrubbed from logs |
| Prompt injection names a device ("send to my other phone") | Outside content is data; device must exist in registry; send/share still needs approval |

## Build order

1. Schema and parser: `devices.toml` + env merge + validation + `devices` table.
2. Read-only API and tools: `GET /v1/devices`, `list_devices`, `device_info`, device index in the prompt.
3. Per-device tokens, `origin_device` on every turn, `last_seen` and status.
4. Pairing and revoke (CLI + API) with action-log entries.
5. Safety Gate device checks and `can_approve` on approvals.
6. Notice routing and the short offline queue.
7. Nodes: tools that run on another device.

## Open questions

- Should the host be allowed to move to another device (failover), or stay fixed in v1?
- How does a headless node pair when it has no screen for the code?
- Do we need device groups ("all home devices") for routing rules?
- Should location (home, work) come from the owner only, or may a trusted GPS device update it?
