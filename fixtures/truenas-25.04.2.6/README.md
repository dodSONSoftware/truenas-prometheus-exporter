# Captured TrueNAS 25.04.2.6 fixtures

This directory is reserved for **captured, sanitized** API payloads from a system independently verified as TrueNAS SCALE 25.04.2.6. No captured payloads have been supplied yet. Creating this directory does not establish appliance compatibility.

Synthetic fixtures remain in `application/tests/fixtures/` and must never be relabeled as captured evidence. Do not contact MEMORY ALPHA or another live appliance to populate this directory without explicit operator authorization for that specific interaction.

## Intake requirements

- Preserve JSON types, optional/missing fields, nulls, units and response/event structure. Do not fill missing values or convert numeric strings during sanitization.
- Remove credentials, tokens, user details, serial numbers, GUIDs and confidential paths. Alias host, pool, interface and device identities consistently. Record fields removed or aliased so tests can distinguish sanitization from actual API omissions.
- Exclude authentication requests entirely. No raw key, password, cookie, session token or unredacted auth response belongs here.
- Use descriptive filenames, for example `pool.query.response.json` and later `reporting.realtime.notification.json`. These names are examples, not existing captures.
- Alongside each supplied fixture, record capture date, operator-confirmed appliance version and how it was established, method/event and parameters, whether the file is a full wire envelope or only a result, sanitization performed, and any uncertainty. Do not include identifying operator details.
- Review files locally for sensitive material before staging. Tests must consume saved files offline and require no credentials or NAS access.

M2 needs a full `pool.query` response. M3 additionally needs version/system and realtime frames, including normal, missing/null and down-link cases where available. Only assert behavior supported by the supplied evidence; do not invent captures for unavailable cases.
