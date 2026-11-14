# Synthetic fixtures

These fixtures are manually constructed from the handoff and versioned schemas. The pool name is fictional; capacity numbers exercise the historical example's integer scale. No fixture is a captured appliance frame or evidence of live compatibility. Scan content is deliberately ignored until M3's callable scan contract is verified. No real credentials, serials, paths or personal metadata are present.

`system.synthetic.json` exercises the pinned system.info selected-field contract with fictional hostname and serial markers. It is not captured appliance data. The serial marker is deliberately included to verify that ignored hardware identity is not printed by check-system.

`realtime.synthetic.json` is a manually constructed full collection_update envelope based on pinned 25.04.2.6 producer code. CPU/core, physical memory, ARC and up/down network examples are fictional. It is not a captured notification or evidence of live subscription authorization. The source's failed_to_connect-only case and malformed/missing/null variants are generated explicitly by offline tests.
