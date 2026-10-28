---
description: Never write todo!/unimplemented!/dbg! in EasyTierUI Rust code, tests included
condition: '(?:^|\n)(?![ \t]*//)[^\n]*?\b(?:todo|unimplemented|dbg)!\s*[(\[{]'
scope: "tool:edit(*.rs), tool:write(*.rs)"
interruptMode: always
---
`todo!()`, `unimplemented!()` and `dbg!()` never land, not even in tests:

- `todo!` / `unimplemented!` are stubs that panic. Finish the implementation; if a real prerequisite is missing (an EasyTier API, a platform capability, a user decision), stop and report it instead of shipping a panicking placeholder.
- `dbg!` is a debugging leftover that prints to stderr. Log through the `log` facade (`log::debug!`) or assert in a test.

Re-issue the change without these macros.
