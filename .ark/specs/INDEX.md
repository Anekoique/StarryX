# Specs

Two layers under this directory.

| Layer       | Path        | Authored by | Read pattern                                                            |
| ----------- | ----------- | ----------- | ----------------------------------------------------------------------- |
| Project     | `project/`  | User        | Read every entry before any task — these are conventions that always apply. |
| Features    | `features/` | Promoted on deep-tier commit | Scan the index, then read only SPECs the task touches. |

Each layer has its own `INDEX.md` — start there.
