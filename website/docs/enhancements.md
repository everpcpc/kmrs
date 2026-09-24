---
title: Enhancements
---

# Enhancements

Improvements kmrs adds on top of Java parity, plus intentional behavior differences. Accepted gaps that are not planned to be closed are listed under [Known limitations](./limitations.md).

## Capabilities the Java version does not have

- **Search**: simplified ↔ traditional Chinese cross-search and CJK boundary unigrams (e.g. `3月` matches `3月的狮子`). Details: [Search](./search.md).
- **Natural sort with a configurable locale**: numbered titles sort by numeric value ("Page 2" < "Page 10", fractions right-aligned so 1.5 > 1.10) under a total order (deterministic pagination boundaries), and the sort locale follows `server.sort-locale` / `KOMGA_SORT_LOCALE` (BCP47, e.g. `zh-CN` for pinyin order). The Java version hard-codes the UCA root and sorts numbered titles lexicographically ("Page 10" < "Page 2").

- **Open databases created by komga-riir**: kmrs adopts databases built by
  komga-riir (a parallel Rust reimplementation), which record migration
  history in sqlx's `_sqlx_migrations` instead of `flyway_schema_history`.
  The Java version cannot open such databases at all. Adoption stamps the
  sqlx-recorded versions (and the Java JDBC data-fix ports from an explicit
  whitelist, which must not run against komga-riir-shaped data) into a
  rebuilt Flyway history; kmrs always applies its own `book_projection`.
  Migrations komga-riir applied beyond a Java komga / kmrs history are also
  absorbed so their DDL is not re-run. Note: stamping endorses komga-riir's
  same-version SQL with kmrs's own checksum — if a same-version migration's
  content ever diverges between the two implementations, the drift is
  undetectable after stamping. This is inherent to the design and accepted
  consciously.

## Behavior differences

Intentional differences from the Java behavior:

- Page order and series book numbering are case-sensitive; the Java natural comparator is case-insensitive.
