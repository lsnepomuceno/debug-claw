# ADR-0014: Reading values over DBGp

- Status: accepted
- Date: 2026-11-07
- Evidence: [S2 report](../00-spikes/reports/s2-logpoints.md), criteria 6 and 7; [S1 report](../00-spikes/reports/s1-path-mapping.md)

## Context

Variables, watches, logpoints and the MCP tools all read values through DBGp `property_get`, `property_value` and `context_get`. [Rule 5 of the performance guide](../../docs/performance.md#rules) says every read is bounded at the source. Spike S2 measured how Xdebug 3.5.3 bounds a read, how it marks a cut, and which expressions it reads by name without eval.

Three findings shape this decision:

- The DBGp limits bound a read only at `max_depth` 1. Below that the size about doubles with each level, and Xdebug follows parent links with no cycle check: one `$this` of the fixture graph was 13.8 MB at depth 20.
- `property_get` returns a wrong value with no error for some expressions that are not plain paths: text after a closing `]` is ignored, and an unquoted key returns the whole array.
- Error 300 means both "not set" and "not a name", so the answer cannot tell the two apart.

## Decision

### Limits sent with every session

At the start of every session the daemon sends `feature_set` with:

| Setting | Value |
|---|---|
| `max_depth` | 1 |
| `max_children` | 32 |
| `max_data` | 512 |

Deeper levels are read on demand by the `fullname` Xdebug reports for each child. The children of one property are paged with `property_get -p N`, 32 per page. A full string is read with `-m 0` for one command, and only when asked.

### Result cap

The daemon cuts every MCP result at 32 KB after converting it, whatever DBGp returned. A cut result says where it was cut and how to get the rest: the next `-p` page for the children of the top property, the `fullname` of each child for anything deeper, or a full read for a cut string. A list is known to be cut when `numchildren` is larger than the number of children returned, and a string when `size` is larger than the length returned.

### Expressions read without eval

An expression is read with `property_get` only when the whole of it matches this grammar. The daemon checks it before sending anything, and does not rely on error 300.

```
path      = base segment*
base      = "$" identifier                         ; includes $this; context 0
          | superglobal                            ; context 1: $_GET, $_POST, $_COOKIE,
                                                   ;   $_SERVER, $_FILES, $_ENV, $_REQUEST, $_SESSION
segment   = "->" identifier
          | "->*" class-name "*" identifier        ; parent private, as Xdebug names it
          | "::" identifier                        ; static property, after $this only
          | "[" integer "]"
          | "[" "'" chars "'" "]" | "[" '"' chars '"' "]"
constant  = identifier                             ; no "$": property_get -c 2
```

Every other expression needs eval: calls, operators, casts, `??`, interpolation, class constants, `Class::$static`, `self::`, `static::`, `::class` and `$GLOBALS`. Whether and when eval is allowed is [O-05](../OPEN.md#o-05-eval-policy).

Writing a value with `property_set` counts as eval, because Xdebug evaluates the value as PHP (S1).

## Alternatives rejected

- Xdebug's defaults (`max_children` 32, `max_data` 1024). One depth 1 read of 32 strings of 5000 characters was 46 816 bytes, over the 32 KB cap. With `max_data` 512 it was 24 928 bytes.
- A deeper default `max_depth`. No `max_children` or `max_data` value bounds a read below depth 1.
- Sending any expression to `property_get` and falling back to eval on error 300. Xdebug answers some non-paths with a wrong value and no error.

## Consequences

- Expanding a tree costs one round trip per level. S2 measured 40 us for a depth 1 read of `$this` on the fixture.
- The `->*Class*name` form in a `fullname` is not valid PHP. A "copy as PHP path" feature has to rewrite it or refuse it.
- The size of the MCP result after conversion to JSON was not measured. The 32 KB cap applies to the converted result.
- S2 did not check objects with `__get`, `ArrayAccess` or `__debugInfo` (whether `property_get` reads stored properties or runs PHP code), superglobals other than `$_GET` and `$_SERVER`, or PHP 8.3. These are design and test items for milestone 1.
- A read of a whole frame (`context_get` with many locals, or the superglobals context, 13 256 bytes at the defaults on `/loop`) can still pass 32 KB. The result cap handles it.
